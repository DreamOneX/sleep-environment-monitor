# Server

This directory is reserved for the measurement ingestion server. Keep server implementation files here rather than in the repository root.

Current contents:

- `post_receiver.py`: minimal local HTTP POST receiver used for firmware upload validation.

The future formal server should replace or supersede the temporary receiver here.

## Temporary Receiver

```bash
python3 server/post_receiver.py
```

It listens on `0.0.0.0:8080` and prints received measurement payloads.
