# Offline cloud-init datasource

`migrate-plan` already told you to reconfigure the datasource. This writes
the plan.

```bash
guestkit cloud-init aws disk.qcow2
guestkit cloud-init azure disk.qcow2 --disable-network
guestkit cloud-init nocloud disk.qcow2 \
  --user-data user-data.yaml \
  --instance-id web01

guestkit plan generate disk.qcow2 -p cloud-init-gcp -o gcp.yaml
guestkit plan apply disk.cloud-init.yaml --vm disk.qcow2 --yes
```

| Target | Guest file |
|---|---|
| aws / ec2 | `/etc/cloud/cloud.cfg.d/99-guestkit-datasource.cfg` with `Ec2` |
| azure | `Azure` |
| gcp / gce | `GCE` |
| openstack | `OpenStack, ConfigDrive` |
| nocloud | `NoCloud` + `/var/lib/cloud/seed/nocloud/{user-data,meta-data}` |

`--disable-network` writes `network: {config: disabled}` so existing guest
NICs are not rewritten on first boot.

Does not call AWS/Azure/GCP APIs. Does not run `cloud-init init` offline.
