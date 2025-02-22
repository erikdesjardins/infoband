# infoband

Windows "DeskBand" displaying cpu/mem/disk/network info.

![](./infoband.png)

## Configuration

On first startup, `infoband` will generate a config file at `%localappdata%\infoband\infoband.json`.

`infoband` does not apply config changes in real time, but it does kill the previous instance on startup. So my usual workflow for tweaking configuration is to repeatedly save the configuration and run `infoband` to see the result.

Use `offset_from_right` to adjust the position. (In units of unscaled pixels. So `"offset_from_right": 200` will produce a 200px offset at 100% scaling, 300px offset at 150% scaling, etc.)

```json
{
  "offset_from_right": 500
}
```

To mute and unmute your mic with a hotkey, populate the `mic_hotkey` section with the desired [Virtual Key Code](https://learn.microsoft.com/en-us/windows/win32/inputdev/virtual-key-codes) and modifiers.

```json
{
  "offset_from_right": 375,
  "mic_hotkey": {
    "virtual_key_code": 67,
    "win": true,
    "ctrl": true,
    "shift": true,
    "alt": true
  }
}
```
