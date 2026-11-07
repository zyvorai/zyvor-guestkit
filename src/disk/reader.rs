// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Disk image reader
//!
//! Pure Rust implementation for reading disk images (raw, qcow2, etc.)

use crate::core::{DiskFormat, Error, Result};
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

/// ioctl constant for getting block device size (Linux x86/x86_64)
#[cfg(target_os = "linux")]
const BLKGETSIZE64: libc::c_ulong = 0x80081272;

/// Disk image reader
pub struct DiskReader {
    file: File,
    format: DiskFormat,
    size: u64,
}

impl DiskReader {
    /// Open a disk image
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path_ref = path.as_ref();
        let mut file = File::open(path_ref).map_err(Error::Io)?;

        // Detect format by reading magic bytes
        let format = Self::detect_format(&mut file)?;

        // Get file size
        // For block devices, metadata().len() doesn't work - need to use ioctl
        let size = if Self::is_block_device(path_ref) {
            // For block devices, use BLKGETSIZE64 ioctl on Linux
            #[cfg(target_os = "linux")]
            {
                use std::os::unix::io::AsRawFd;

                let mut size_bytes: u64 = 0;
                let result = unsafe {
                    libc::ioctl(
                        file.as_raw_fd(),
                        BLKGETSIZE64 as _,
                        &mut size_bytes as *mut u64,
                    )
                };

                if result == 0 {
                    size_bytes
                } else {
                    return Err(Error::Io(std::io::Error::last_os_error()));
                }
            }
            #[cfg(not(target_os = "linux"))]
            {
                // Fallback for non-Linux: try seeking (might not work)
                file.seek(SeekFrom::End(0)).map_err(Error::Io)?
            }
        } else {
            // For regular files, use metadata
            file.metadata().map_err(Error::Io)?.len()
        };

        // Reset to start
        file.seek(SeekFrom::Start(0)).map_err(Error::Io)?;

        Ok(Self { file, format, size })
    }

    /// Check if path is a block device
    fn is_block_device(path: &Path) -> bool {
        #[cfg(unix)]
        {
            use std::os::unix::fs::FileTypeExt;
            if let Ok(metadata) = std::fs::metadata(path) {
                return metadata.file_type().is_block_device();
            }
        }
        false
    }

    /// Detect on-disk image format from magic bytes (not file extension).
    pub fn detect_image_format<P: AsRef<Path>>(path: P) -> Result<DiskFormat> {
        let mut file = File::open(path).map_err(Error::Io)?;
        Self::detect_format(&mut file)
    }

    /// Detect disk image format from magic bytes
    fn detect_format(file: &mut File) -> Result<DiskFormat> {
        let mut header = [0u8; 512];
        file.seek(SeekFrom::Start(0)).map_err(Error::Io)?;

        // Use read() in a loop instead of read_exact(): block devices and
        // short files may return fewer bytes than requested.
        let mut bytes_read = 0;
        while bytes_read < header.len() {
            let n = file.read(&mut header[bytes_read..]).map_err(Error::Io)?;
            if n == 0 {
                break;
            }
            bytes_read += n;
        }

        Ok(Self::detect_format_from_header(&header[..bytes_read]))
    }

    /// Classify an image from its first bytes (up to 512).
    fn detect_format_from_header(header: &[u8]) -> DiskFormat {
        if header.len() < 4 {
            // Not enough data to detect format, assume raw
            return DiskFormat::Raw;
        }

        // QCOW2 magic: "QFI\xfb"
        if header.starts_with(b"QFI\xfb") {
            return DiskFormat::Qcow2;
        }

        // VMDK: sparse extent ("KDMV"), ESX COWD extent, or text descriptor
        if header.starts_with(b"KDMV")
            || header.starts_with(b"COWD")
            || header.starts_with(b"# Disk DescriptorFile")
        {
            return DiskFormat::Vmdk;
        }

        // VHDX file type identifier at offset 0
        if header.starts_with(b"vhdxfile") {
            return DiskFormat::Vhdx;
        }

        // Dynamic and differencing VHDs start with a copy of the "conectix"
        // footer. (Fixed VHDs only carry it at the end and read correctly as raw.)
        if header.starts_with(b"conectix") {
            return DiskFormat::Vhd;
        }

        // VDI: signature 0xBEDA107F (little-endian) at offset 0x40
        if header.len() >= 0x44 && header[0x40..0x44] == [0x7f, 0x10, 0xda, 0xbe] {
            return DiskFormat::Vdi;
        }

        // Default to raw if no magic found
        DiskFormat::Raw
    }

    /// Read bytes at offset
    pub fn read_at(&mut self, offset: u64, buf: &mut [u8]) -> Result<usize> {
        self.file.seek(SeekFrom::Start(offset)).map_err(Error::Io)?;
        self.file.read(buf).map_err(Error::Io)
    }

    /// Get disk format
    pub fn format(&self) -> &DiskFormat {
        &self.format
    }

    /// Get disk size
    pub fn size(&self) -> u64 {
        self.size
    }

    /// Read exact bytes at offset
    pub fn read_exact_at(&mut self, offset: u64, buf: &mut [u8]) -> Result<()> {
        self.file.seek(SeekFrom::Start(offset)).map_err(Error::Io)?;

        // For block devices, we might need to read in chunks
        let mut total_read = 0;
        while total_read < buf.len() {
            match self.file.read(&mut buf[total_read..]) {
                Ok(0) => {
                    return Err(Error::Io(std::io::Error::new(
                        std::io::ErrorKind::UnexpectedEof,
                        format!(
                            "Failed to read {} bytes at offset {}, only got {} bytes",
                            buf.len(),
                            offset,
                            total_read
                        ),
                    )));
                }
                Ok(n) => total_read += n,
                Err(e) => return Err(Error::Io(e)),
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_block_device_detection() {
        // Regular files should not be detected as block devices
        assert!(!DiskReader::is_block_device(std::path::Path::new(
            "/etc/hosts"
        )));
        assert!(!DiskReader::is_block_device(std::path::Path::new(
            "/nonexistent"
        )));
    }

    fn header_with(prefix: &[u8]) -> Vec<u8> {
        let mut header = vec![0u8; 512];
        header[..prefix.len()].copy_from_slice(prefix);
        header
    }

    #[test]
    fn test_detect_format_from_header() {
        let detect = DiskReader::detect_format_from_header;

        assert_eq!(detect(&header_with(b"QFI\xfb")), DiskFormat::Qcow2);
        assert_eq!(detect(&header_with(b"KDMV")), DiskFormat::Vmdk);
        assert_eq!(detect(&header_with(b"COWD")), DiskFormat::Vmdk);
        assert_eq!(
            detect(&header_with(b"# Disk DescriptorFile\nversion=1\n")),
            DiskFormat::Vmdk
        );
        assert_eq!(detect(&header_with(b"vhdxfile")), DiskFormat::Vhdx);
        assert_eq!(detect(&header_with(b"conectix")), DiskFormat::Vhd);

        let mut vdi = header_with(b"<<< Oracle VM VirtualBox Disk Image >>>\n");
        vdi[0x40..0x44].copy_from_slice(&[0x7f, 0x10, 0xda, 0xbe]);
        assert_eq!(detect(&vdi), DiskFormat::Vdi);

        let mut mbr = vec![0u8; 512];
        mbr[510] = 0x55;
        mbr[511] = 0xAA;
        assert_eq!(detect(&mbr), DiskFormat::Raw);
        assert_eq!(detect(b"QF"), DiskFormat::Raw);
    }

    #[test]
    fn test_detect_image_format_reads_file() {
        let mut file = tempfile::NamedTempFile::new().unwrap();
        std::io::Write::write_all(&mut file, &header_with(b"vhdxfile")).unwrap();
        assert_eq!(
            DiskReader::detect_image_format(file.path()).unwrap(),
            DiskFormat::Vhdx
        );
    }
}
