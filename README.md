<div align="center">

<img width="128px" src="https://github.com/user-attachments/assets/c3320aa3-4783-4aaa-a965-0124439c630f" />
<h1><b>Senn</b></h1>

Self-hosted Docker panel. One panel, many hosts, plain compose folders.

</div>
<br>

<img width="3840" height="2160" alt="senn" src="https://github.com/user-attachments/assets/b81bfe92-1279-4a54-8682-48c7307c4f26" />

<br>

The panel never touches Docker. Every host runs `senn-agent` (a systemd service) that connects out to the panel over WebSocket, so hosts need no open port and the panel is the only thing exposed (behind https).

## Run the panel

```yml
services:
  senn:
    image: ghcr.io/0pandadev/senn:latest
    ports:
      - "3000:3000"
    environment:
      SENN_BASE_URL: "" # public https URL of the panel, agents connect back to it, e.g. https://senn.example.com
      SENN_TRUST_PROXY: 1 # reverse proxies in front of senn, 0 when exposed directly
      SENN_METRICS_URL: http://victoriametrics:8428 # empty disables metrics
    volumes:
      - senn_data:/app/data
    depends_on:
      - victoriametrics
    restart: unless-stopped

  victoriametrics:
    image: victoriametrics/victoria-metrics:v1.157.0
    command:
      - -storageDataPath=/storage
      - -retentionPeriod=${SENN_METRICS_RETENTION:-12} # months, add a unit suffix like 30d or 2y to change it
      - -memory.allowedBytes=256MB
      - -search.latencyOffset=1s
    volumes:
      - vm_data:/storage
    restart: unless-stopped

volumes:
  senn_data:
  vm_data:

```

```sh
docker compose up -d
```

Open it, create the one account on first start, then add agents.

If storage is down the panel buffers samples in memory for 5 minutes, then drops them with a warning.

## Stacks

A stack is a subfolder of the stacks path with a compose file (`compose.yaml`, `compose.yml`, `docker-compose.yaml`, `docker-compose.yml`). Files stay the source of truth, edit them in the panel or on the host.

Image updates are checked against the registry digest. Choose what happens when one is found, in the panel: the Updates column on the Stacks page sets a whole stack, and the one on a stack or Containers page sets a single service (Inherit follows the stack).

- Notify only (default): the update shows up as a tag and in notifications, you apply it.
- Auto update: the agent pulls and recreates the service as soon as a scheduled check finds an update.
- Off: updates for it are ignored. No registry check, no tag, no notification, and Update all skips it.

The agent checks the registry on a schedule (every 6 hours by default) and the agent settings dialog sets the interval and the default for services with nothing chosen. A compose label still works as a fallback when nothing is set in the panel:

```yaml
labels:
  senn.update: auto   # auto | notify (default) | off
```

