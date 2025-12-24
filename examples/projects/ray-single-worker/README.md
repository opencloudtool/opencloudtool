# Ray cluster

https://www.ray.io/

The deployment will work only on `t2.large` instance

## Deploy a cluster

```bash
cargo run -p oct-cli apply
```

## Submit a job

```bash
export RAY_ADDRESS=http://<head_public_ip>:<head_port>

uv run ray job submit --working-dir . -- python main.py
```
