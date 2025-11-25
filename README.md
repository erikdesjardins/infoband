# infoband

Windows "DeskBand" displaying cpu/mem/disk/network info.

![](./infoband.png)

## Configuration

On first startup, `infoband` will generate a config file at `%localappdata%\infoband\infoband.json`.

`infoband` does not apply config changes in real time, but it does kill the previous instance on startup. So my usual workflow for tweaking configuration is to repeatedly save the configuration and run `infoband` to see the result.

To mute and unmute your mic with a hotkey, populate the `mic_hotkey` section with the desired [Virtual Key Code](https://learn.microsoft.com/en-us/windows/win32/inputdev/virtual-key-codes) and modifiers.

```json
{
  "mic_hotkey": {
    "virtual_key_code": 67,
    "win": true,
    "ctrl": true,
    "shift": true,
    "alt": true
  }
}
```
