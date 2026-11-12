# Copyright 2026 Zyvor AI Labs · https://zyvor.dev
# SPDX-License-Identifier: Apache-2.0
FROM python:3.12-alpine
# serve-https.py shells out to `openssl req` to mint a self-signed cert at start-up;
# python:alpine does not ship the binary.
RUN apk add --no-cache openssl
WORKDIR /ui
COPY . /ui
EXPOSE 8443
CMD ["python3","serve-https.py","--port","8443","--dir","/ui"]
