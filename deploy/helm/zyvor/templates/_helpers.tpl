{{- define "zyvor.redisUrl" -}}
{{- if .Values.redis.password -}}
redis://:{{ .Values.redis.password }}@redis:6379
{{- else -}}
redis://redis:6379
{{- end -}}
{{- end }}

{{- define "zyvor.vmVolume" -}}
{{- if .Values.persistence.vmImages.hostPath }}
- name: vm-images
  hostPath:
    path: {{ .Values.persistence.vmImages.hostPath }}
    type: DirectoryOrCreate
{{- else }}
- name: vm-images
  persistentVolumeClaim:
    claimName: zyvor-image-store
{{- end }}
{{- end }}

{{- define "zyvor.vmVolumeMount" -}}
- name: vm-images
  mountPath: /var/lib/zyvor/images
{{- end }}

{{- define "zyvor.kubevirtStorageVolume" -}}
{{- if .Values.persistence.kubevirtLocalPath }}
- name: kubevirt-local-storage
  hostPath:
    path: {{ .Values.persistence.kubevirtLocalPath }}
    type: DirectoryOrCreate
{{- end }}
{{- end }}

{{- define "zyvor.kubevirtStorageVolumeMount" -}}
{{- if .Values.persistence.kubevirtLocalPath }}
- name: kubevirt-local-storage
  mountPath: {{ .Values.persistence.kubevirtLocalPath }}
  readOnly: true
{{- end }}
{{- end }}

{{- define "zyvor.agentCaVolumeMount" -}}
- name: vm-images
  mountPath: /var/lib/zyvor/agent-ca
  subPath: agent-ca
{{- end }}

{{- define "zyvor.vmtoolsBundleBaseUrl" -}}
{{- if .Values.vmtools.bundle.baseUrl -}}
{{- trimSuffix "/" .Values.vmtools.bundle.baseUrl -}}
{{- else if .Values.zyvorApi.zeusPublicUrl -}}
{{- printf "%s/api/v1/vmtools/bundle" (trimSuffix "/" .Values.zyvorApi.zeusPublicUrl) -}}
{{- else -}}
http://minio:9000/vmtools
{{- end -}}
{{- end }}
