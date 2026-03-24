# Virtual Pet

A virtual pet game for ESP32 with MicroPython, featuring an SSD1306 OLED display and button controls.

![catstars](https://github.com/user-attachments/assets/2ffc652a-f392-42e7-9a13-d7fb91f3770d)

## How the Pet Works

### Stats

The pet tracks 18 stats, split into tiers by how quickly they change:

| Tier | Stats | Change rate |
|------|-------|-------------|
| Rapid | health, fullness, energy, comfort, playfulness, focus | Daily |
| Medium | fulfillment, cleanliness, intelligence, maturity, affection | Weekly |
| Slow | fitness, serenity | Monthly |
| Trait | courage, loyalty, mischievousness, curiosity, sociability | Nearly fixed |

All stats sit on a 0-100 scale. **Health** is never set directly; it's a weighted average of fitness, fullness, energy, cleanliness, comfort, affection, fulfillment, focus, intelligence, and playfulness, recomputed after every behavior completes.

Each pet gets a unique 64-bit seed at creation. That seed deterministically derives balanced personality offsets (up to +/-10) for the five trait stats, so every pet feels distinct without any individual pet being universally happier or sadder than another.

Stat changes use **asymptotic damping**: a stat near its ceiling resists further increases, and a stat near the floor resists further decreases. This keeps rewards feeling meaningful throughout the full 0–100 range.

### Behaviors

The pet runs one behavior at a time. Behaviors are lazy-loaded and their modules are unloaded from memory after completion, keeping RAM usage low. The full list includes:

`sleeping`, `napping`, `stretching`, `kneading`, `lounging`, `investigating`, `observing`, `chattering`, `zoomies`, `vocalizing`, `self_grooming`, `being_groomed`, `hunting`, `gift_bringing`, `pacing`, `sulking`, `mischief`, `hiding`, `training`, `playing`, `affection`, `attention`, `eating`, `startled`, `meandering`, `go_to`

After each behavior finishes, the next one is chosen automatically:

1. Each behavior defines a `can_trigger` condition (stat thresholds, time of day, location, etc.).
2. Eligible behaviors are given a random priority draw (lower is better).
3. Recently completed behaviors get a priority penalty to prevent loops.
4. The best few behaviors are binned together and one is chosen at random from the top bin.

Personality traits feed directly into the selection. A high-mischievousness pet triggers `mischief` more often; a low-courage pet is more prone to `hiding` and `startled`.

High serenity adds a chance to skip the selection entirely and stay idle; a content pet is happy doing nothing.

### Coins and the store

Coins are the in-game currency. They are earned by:

- **Minigames**: Zoomies, Snake, Maze, Memory, Hanjie, Breakout, and Tic-tac-toe all award coins on completion, scaled by how well the player did.
- **Hunting**: each successful hunt awards 1-3 coins at random.

Coins are spent at the **store**, which is accessible from the main scene menu. The store sells:

| Category | Items | Cost |
|----------|-------|------|
| **Meals** | Chicken, Salmon, Tuna, Shrimp, Trout, Herring, Haddock, Cod, Turkey, Kibble, Beef, Lamb, Liver | 4-8c per 5 uses |
| **Snacks** | Treats, Nuggets, Puree, Milk, Chew Sticks, Fish Bytes, Eggs, Pumpkin, Carrots | 2-4c per 5 uses |
| **Toys** | String (5c), Feather (8c), Yarn Ball (10c), Laser Pointer (15c) | one-time purchase |

Food is consumed by feeding the pet from the main scene menu and depletes by one use per feeding. Different foods grant different stat bonuses: meals primarily restore fullness and energy, while snacks tend to boost comfort and affection. Toys can be used to trigger the playing behavior.

### Location rewards

The pet can roam between five scenes: **inside**, **bedroom**, **kitchen**, **outside**, and **treehouse**. Location is tracked in `context.last_main_scene`.

The pet navigates autonomously using the `go_to` behavior. At the end of each behavior, there is a small base chance (~8%) of walking to a new room, boosted by relevant needs:

- **Hungry**: more likely to head to the kitchen
- **Tired or uncomfortable**: more likely to head to the bedroom
- **Bad weather**: strongly discourages trips outside or to the treehouse

Each location modifies the stat rewards from behaviors:

| Location | Effect |
|----------|--------|
| **Bedroom** | Sleeping grants +30% energy and +25% comfort. Sleep/nap trigger thresholds are raised (the pet falls asleep more readily). |
| **Kitchen** | Eating grants +20% fullness and energy. |
| **Outside / Treehouse** | Hunting grants +50% fitness and bonus fulfillment. |
| **Outside / Treehouse (bad weather)** | Sleeping or lounging in rain, storms, or snow incurs a comfort penalty. |
| **Inside / Outside / Treehouse** | Lounging grants +30% comfort (vs. a bedroom baseline). |

### Weather

Weather follows a **deterministic Markov chain** seeded from the pet's unique seed, so each pet has its own distinct long-term weather trajectory that is reproducible across saves.

Possible states: Clear, Cloudy, Overcast, Windy, Rain, Storm, and Snow (Fall/Winter only, transitioning from Overcast). Each state lasts between 30 and 300 in-game minutes before transitioning.

Weather influences behavior in several ways:

- **Scene navigation**: rain, storms, and snow reduce the pet's desire to go outside or to the treehouse.
- **Outdoor sleep/lounge**: bad weather while outside applies a comfort penalty on completion.
- **Forecast screen**: because the weather is fully deterministic, a 72-hour forecast can be computed ahead of time without any randomness.

### WiFi home detection

> [!NOTE]
> **Disabled by default.** Set `WIFI_ENABLED = True` in `config.py` to enable. See the RAM warning below before doing so.

The ESP32's WiFi radio is used to determine whether the pet is at its familiar home location. A scan runs **once at boot** (while the loading screen is shown) and can be triggered manually from the debug WiFi scene.

Two lists of access points are maintained:

- **`wifi_familiar`**: up to 16 well-known APs (persisted to flash). An AP here means the pet considers this a home location.
- **`wifi_recent`**: up to 8 candidate APs (persisted). New APs land here first and are promoted to familiar after being seen at least 5 times. Entries that aren't seen decay by 0.25 per scan and are pruned when they reach zero.

`context.in_familiar_location` is set to `True` whenever at least one familiar AP is visible. This flag affects multiple behaviors:

| When familiar | When unfamiliar |
|---------------|-----------------|
| Zoomies and playing are more likely | Investigating, pacing, and startled are more likely |
| Lounging is more likely; grants +1.5 serenity and +15% comfort | Sulking and hiding are more likely |
| Sleeping grants +3 serenity | Sleeping loses 2 serenity and 15% comfort |
| Hiding is much less likely | Hiding is much more likely |

The intent is that a pet left at home is calmer, sleeps better, and plays more freely, while a pet taken somewhere unfamiliar becomes more anxious and restless.

> [!WARNING]
> #### RAM cost of enabling WiFi
>
> Enabling wifi will make the device freeze within an hour or two.
> 
> On ESP32-C3/C6, all SRAM is shared; there is no separate "WiFi RAM." When the WiFi driver initialises (`network.WLAN(...).active(True)`), the ESP-IDF stack allocates internal buffers (TX/RX queues, the lwIP network stack, control structures) that are never returned, even after `wlan.active(False)` and garbage collection. This is because `active(False)` only stops the radio; it does not call `esp_wifi_deinit()`, and MicroPython's network API does not expose deinit.
> 
> The practical result: enabling WiFi permanently reduces available heap for the rest of the boot session. On devices already running close to the memory limit this is enough to cause allocation failures during scene changes or behavior loads, typically within an hour of boot. With `WIFI_ENABLED = False` the devices run indefinitely without issue.

## Controls

- **D-pad**: Navigate / Move character
- **A/B buttons**: Action buttons
- **Menu buttons**: Additional functions

![spookycat](https://github.com/user-attachments/assets/c1f8b6eb-b90c-46ad-b652-80093db97f83)

## Setup

### Hardware Requirements

- **ESP32-C6 SuperMini** OR **ESP32-C3** development board
- **SSD1306 OLED Display** (128x64, I2C)
- **8 Push Buttons** for input

### Software Requirements

- `mpremote` installed (`pip install mpremote`)

### Board Configuration

The project supports both ESP32-C6 and ESP32-C3 boards. To configure for your board:

1. Open `src/config.py`
2. Set `BOARD_TYPE` to either `"ESP32-C6"` or `"ESP32-C3"`

```python
# In src/config.py
BOARD_TYPE = "ESP32-C6"  # Change to "ESP32-C3" for ESP32-C3 board
```

### Wiring

Choose the wiring diagram for your board. Each button connects between GPIO pin and GND (internal pull-ups enabled).

#### ESP32-C6 Wiring

**Display (I2C):**
|Display Pin | ESP32-C6 Pin |
|--------|----------|
|VCC | 3V3 |
|GND | GND |
|SDA | GPIO4 |
|SCL | GPIO7 |

**Buttons:**
| Button | GPIO Pin |
|--------|----------|
| UP     | GPIO0    |
| DOWN   | GPIO1    |
| LEFT   | GPIO2    |
| RIGHT  | GPIO3    |
| A      | GPIO20   |
| B      | GPIO19   |
| MENU1  | GPIO18   |
| MENU2  | GPIO14   |

#### ESP32-C3 Wiring

**Display (I2C):**
|Display Pin | ESP32-C3 Pin |
|--------|----------|
|VCC | 3V3 |
|GND | GND |
|SDA | GPIO6 |
|SCL | GPIO7 |

**Buttons:**
| Button | GPIO Pin |
|--------|----------|
| UP     | GPIO0    |
| DOWN   | GPIO1    |
| LEFT   | GPIO2    |
| RIGHT  | GPIO3    |
| A      | GPIO4    |
| B      | GPIO5    |
| MENU1   | GPIO10  |
| MENU2   | GPIO11  |

> **Note:** The ESP32-C3 configuration avoids strapping pins (GPIO8, GPIO9) to prevent boot issues.

## Installation

### 1. Flash MicroPython (if not already done)

**For ESP32-C6:**
```bash
esptool.py --chip esp32c6 --port /dev/cu.usbmodem* erase_flash
esptool.py --chip esp32c6 --port /dev/cu.usbmodem* write_flash -z 0x0 ESP32_GENERIC_C6-*.bin
```

**For ESP32-C3:**
```bash
esptool.py --chip esp32c3 --port /dev/cu.usbmodem* erase_flash
esptool.py --chip esp32c3 --port /dev/cu.usbmodem* write_flash -z 0x0 ESP32_GENERIC_C3-*.bin
```

> Download MicroPython firmware from [micropython.org/download](https://micropython.org/download/)

### 2. Configure Board Type

Before uploading, set your board type in `src/config.py`:
```python
BOARD_TYPE = "ESP32-C6"  # or "ESP32-C3"
```

### 3. Install SSD1306 Library

```bash
mpremote mip install ssd1306
```

## Development Workflow

For the fastest iteration during development, use the `dev.sh` script which compiles Python to bytecode and runs via `mpremote mount`:

```bash
./dev.sh
```

This script:
- Compiles all `.py` files in `src/` to `.mpy` bytecode in `build/`
- Mounts the `build/` directory on the device
- Runs the game

Using precompiled `.mpy` files provides faster startup and slightly lower RAM usage compared to raw `.py` files.

> [!NOTE]
> Requires `mpy-cross` (`pip install mpy-cross`) and `mpremote` (`pip install mpremote`).
> Any libraries used (like `ssd1306`) must already be installed on the device.

## Scripts

### test_hardware.sh

Verifies that your hardware is working correctly:

```bash
./test_hardware.sh
```

This script:
- Resets the device
- Scans I2C to confirm the display is detected
- Enters an interactive button test (press buttons to see them register, Ctrl+C to exit)

Run this first when setting up a new device or debugging hardware issues.

### upload.sh

Deploys the project to the ESP32's flash storage:

```bash
./upload.sh [port]
```

This script:
- Installs the `ssd1306` library via `mip`
- Compiles all `.py` files to `.mpy` bytecode
- Cleans existing files from the device (preserves `lib/`)
- Uploads compiled `.mpy` files and `boot.py` to the device

Use this when you want the pet to run standalone without a laptop connection.

## Running the Game

After uploading, the game starts automatically on power-up or reset.

**To enter REPL mode instead:** Hold **A+B buttons** while powering on or pressing reset. This skips auto-run so `mpremote` can connect.

To manually start the game from REPL:

```bash
mpremote
>>> import main
>>> main.main()
```

## Troubleshooting

### "could not enter raw repl" error

If you see `mpremote.transport.TransportError: could not enter raw repl` when running `./dev.sh` or other mpremote commands, it means `boot.py` is on the device and auto-running the game, blocking mpremote from connecting.

**To fix this:**

Either press A + B while `./dev.sh` to interrupt the boot sequence.

Or, to remove the `boot.py` file so that it doesn't activate:

1. Run `mpremote` to connect to the device
2. Press **Ctrl+C** to interrupt the running game
3. Press **Ctrl+B** to exit raw REPL and enter friendly REPL
4. Remove boot.py:
   ```python
   import os
   os.remove('boot.py')
   ```
5. Press **Ctrl+X** to exit mpremote

Now `./dev.sh` should work again.

### Monitoring serial output without interrupting the game

To watch `print()` output from a running game without sending Ctrl+C or triggering a reset:

**macOS:**
```bash
screen /dev/cu.usbmodem* 115200
```

**Linux:**
```bash
screen /dev/ttyACM0 115200
```

If the glob doesn't match (or you have multiple devices), find the exact port first:
- macOS: `ls /dev/cu.*`
- Linux: `ls /dev/ttyACM*` or `ls /dev/ttyUSB*`

Press **Ctrl+A then K** to exit `screen`.

This is useful after a reboot (e.g. from a context save) breaks an mpremote session; the game is still running and its output is still on the serial port.

## Contributing

It's helpful to open an issue prior to making a PR to allow discussion on the changes.

It's also helpful to keep PRs small and targeted.
