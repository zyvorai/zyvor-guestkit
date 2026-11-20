# SELinux relabel, sysprep, BitLocker escrow

Offline cutover prep. Apply with the existing planner:

```bash
guestkit plan apply plan.yaml --vm disk.qcow2 --yes
```

## SELinux

```bash
guestkit selinux-relabel rhel.qcow2
# → rhel.selinux-relabel.yaml  (FileWrite /.autorelabel)

guestkit plan generate rhel.qcow2 -p selinux-relabel -o relabel.yaml
```

Does **not** run `restorecon` offline. Next boot does the relabel.

## Windows sysprep

```bash
guestkit sysprep win.qcow2 --hostname WEB02
guestkit sysprep win.qcow2 --hostname WEB02 --no-firstboot   # unattend only
```

Writes:

- `/Windows/System32/Sysprep/unattend.xml` (persist drivers + ComputerName)
- `/Windows/Setup/Scripts/SetupComplete.cmd`
- `/GuestKit/run-sysprep.flag` unless `--no-firstboot`

`sysprep.exe /generalize /oobe /quit` runs **at first boot**, not while the
disk is mounted on the host.

## BitLocker

GuestKit will not decrypt a volume offline. Passport still hard-blocks
active BitLocker. This command only escrows the key the operator already
has.

```bash
guestkit bitlocker status win.qcow2
guestkit bitlocker escrow win.qcow2 --key-file recovery.txt
# → win.bitlocker-escrow.json  (0600, SHA-256 only)
# → win.bitlocker-plan.yaml    (guest marker, no secret)

guestkit bitlocker escrow win.qcow2 --key-file recovery.txt --include-secret
```

`--include-secret` stores the raw password in the **host** JSON. Never
in the guest.
