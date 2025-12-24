# CS2 server

### 1. Retrieve Your SRCDS Token

A Steam Game Server Login Token (SRCDS_TOKEN) is required to host a public CS2 server.

Get SRCDS token here:  
[Steam Game Server Account Management](https://steamcommunity.com/dev/managegameservers)

- App ID: `730`

### 2. Deploy a server

```bash
export SRCDS_TOKEN=YOUR_SRCDS_TOKEN

cargo run -p oct-cli apply
```

### 3. Connect to server

- Launch CS2
- Enable developer console in settings:
  - Press ~ and type:
    `connect YOUR_EXTERNAL_IP:80`
