# Single host multi services with domains

This project demonstrates how to run multiple services on a single host and assign domains to them.

The domains should be manually assigned to the single host by adding A records to the host's DNS.

## Examples of A records

```
app_1.instance_1.test.opencloudtool.com.  A  <HOST_IP>
app_2.instance_1.test.opencloudtool.com.  A  <HOST_IP>
```

where `<HOST_IP>` is the IP address of the host.
