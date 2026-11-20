# Cutover bundle (one-shot)

```bash
guestkit gate --image disk.qcow2 --fail-below 80 --rego policies/cutover.rego
guestkit gate --passport passport.json --sbom-old a.spdx.json --sbom-new b.spdx.json

guestkit selinux-relabel rhel.qcow2
guestkit sysprep win.qcow2 --hostname WEB02
guestkit bitlocker escrow win.qcow2 --key-file recovery.txt

guestkit cloud-profile aws --image disk.qcow2 --strict
guestkit policy rego --rego policies/cutover.rego --input passport.json --fail
guestkit cloud-init aws disk.qcow2
guestkit virtio-initramfs disk.qcow2 --dracut

guestkit agent-sign keygen --seed seed.hex --public pub.hex   # --features agent
guestkit agent-sign sign manifest.json -o manifest.sig
guestkit agent-sign verify manifest.json --signature manifest.sig

virtctl guestkit resolve myvm -n default
virtctl guestkit doctor myvm -n default          # uses hostDisk if present
virtctl guestkit gate myvm --fail-below 80
```

PVC contents are never mounted by the plugin. Pass `--image` after you copy the volume out.

Initramfs: FileWrite of dracut/initramfs-tools drop-in + `/GuestKit/rebuild-initramfs.flag`. Rebuild is first-boot, not offline.
