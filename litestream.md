# Litestream notes

## Restore
```sh
MINIO_MACHINE_ID=...

flyctl proxy 9000 $MINIO_MACHINE_ID.vm.stargazers.internal
export LITESTREAM_ACCESS_KEY_ID=minioadmin
export LITESTREAM_SECRET_ACCESS_KEY=minioadmin
litestream restore -o restored.sqlite s3://litestream-bucket.localhost:9000/litestream/obelisk
```
