# Catode32 - A virtual pet for your ESP32

![catstars](https://github.com/user-attachments/assets/2ffc652a-f392-42e7-9a13-d7fb91f3770d)

![spookycat](https://github.com/user-attachments/assets/c1f8b6eb-b90c-46ad-b652-80093db97f83)
## Pet Features
- [Pet Care](#pet-care)
- [Behaviors](#pet-behavior)
- [Minigames](#minigames)
- [In-Game Store](#in-game-store)
- [Locations](#locations)
- [Weather](#weather)
- [Vacations](#vacations)
- [Gardening](#gardening)
- [Playdates](#playdates)
- [Home Comfort](#home-comfort)
- [Sickness](#sickness)

### Pet Care
Your pet needs your help to have a healthy, fulfilling, affectionate life.

Your pet has 18 stats which change over time, and they change at different rates.
| Tier | Stats | Change rate |
|------|-------|-------------|
| Rapid | health, fullness, energy, comfort, playfulness, focus | ~Daily |
| Medium | fulfillment, cleanliness, intelligence, maturity, affection | ~Weekly |
| Slow | fitness, serenity | ~Monthly |
| Slowest | courage, loyalty, mischievousness, curiosity, sociability | Very slowly |

All stats sit on a 0-100 scale. Health is never set directly; it's a weighted average of some of the other stats.

To care for your pet, you'll want to:
- keep them well fed with varied meals
- give them affection (pets, scratches, kisses)
- groom them from time to time
- buy them toys and play with them regularly
- gently train their behavior
- play minigames with them
- take them on trips
- and keep their environment interesting with healthy plants

Your pet will help communicate some of these needs through vocalizations. You can also go to the pet stats page to see them all at any time:

![Stats](https://github.com/user-attachments/assets/5c1b3411-8439-4798-8d96-3da26b280524)


### Pet Behavior
Your pet will exhibit various behaviors over time, specifically you'll see them:
`sleeping`, `napping`, `stretching`, `kneading`, `lounging`, `investigating`, `observing`, `chattering`, `zoomies`, `vocalizing`, `self_grooming`, `being_groomed`, `hunting`, `gift_bringing`, `pacing`, `sulking`, `mischief`, `hiding`, `training`, `playing`, `affection`, `attention`, `eating`, `startled`, `meandering`

After finishing, each behavior transitions to a new behavior. The next behavior is selected based on the pets current needs, which behaviors have been exhibited recently, and a bit of randomness.

![Behaviors](https://github.com/user-attachments/assets/97896a35-8ff2-4229-857f-e3466186c84a)


By observing your pet's behaviors you can better understand your pet's needs. If they're sulking or vocalizing that they're bored then perhaps you should play with them or show them some affection. If they're looking a bit upset or vocalizing about food then try going to the kitchen to feed them.

Often they'll be lounging around, napping, or just enjoying their environment.

### Minigames
There are several minigames to keep both you and your pet occupied.

Playing these games provides different stat rewards for your pet, depending on the game type. And they provide coins which you can spend at the in-game store to help care for your pet even better.

The rewards for each game are related to the type of game itself. For example, puzzle games are likely to reward intelligence gains. Action games are more likely to provide fitness gains (and probably some energy losses!) Each game is a bit different, and the rewards are also scaled by how long you play and how successful you are in it. 

### In-Game Store

![In-game store](https://github.com/user-attachments/assets/0920c266-b360-4649-ad1e-e2566e161a54)

You can earn coins through the minigames, and sometimes your pet will find a few coins randomly when they're in the mood to do a little hunting.

These coins can be spent at the in-game store to help you care for your pet.

At the store you can buy:
- Meals
	- Kibble, Cod, Haddock, Trout, Shrimp, Herring, Turkey, Tuna, Salmon, Chicken, Liver, Beef, Lamb
- Snacks
	- Carrots, Pumpkin, Treats, Fish Bytes, Eggs, Nuggets, Milk, Chew Sticks, Puree 
- Toys
	- String, Feather, Yarn Ball, Laser Pointer
- Gardening supplies
	- Various sized pots, Seeds (Grass, Fresia, Sunflower, Roses), Spade, Watering Can, Fertilizer
- Care Services
	- Professional Grooming, Professional Training
- Vacations
	- Trip to the Park, Forest, Aquarium, Beach

Your pet will appreciate variety in their meals and snacks, and they'll be enriched by exposure to new toys and locations. Adding plants to your pet's home, and keeping those plants healthy, will give a big boost to your pet's mood and life!

### Locations

Within your pet's home there are a few different spaces for them to hang out. They are the:
- Living room
- Bedroom
- Kitchen
- Outside (Back yard)
- Treehouse

Some of these have special perks. For example, playing with your pet outside or in the living room provides more satisfaction than playing with them in the kitchen. Likewise, feeding them in the kitchen gives them a bit more satisfaction from food than feeding them in the bedroom. And going to the bedroom when the pet's energy is low will encourage them to sleep or nap earlier than they would otherwise (and they'll get a bigger energy and comfort boost for sleeping there too!) Lounging outside or in the treehouse or the living room will be a bit more serene for your pet than lounging in the kitchen, etc...

You can choose to take your pet to a different location, and sometimes they'll decide to go to different locations on their own.

Beyond those at-home locations, there are also some external locations such as the park, forest, aquarium, and beach which you can visit with your pet by taking vacations via the store.

### Weather

There's a dynamic weather system that progresses over time. The weather can be one of: Clear, Cloudy, Overcast, Windy, Rain, Storm, or Snow. These transition from one to another in sensible ways (i.e., an overcast day might clear up or might start to rain.)

From the Forecast page in the game you can see what the weather will likely be for the next few hours and days.

The weather has some effects on your pet. For example, you don't want to let them sit outside in the rain or their comfort will rapidly plummet!

And while you have a chance to see a shooting star or two each night, you might see a forecast for a meteor shower with lots of them!

### Vacations

![parkvacation](https://github.com/user-attachments/assets/647eb3cb-26e4-4be5-b6ac-8b1a32d0783b)

If you save up some coins you can take your pet on different vacations. Each one will give some different rewards to your pet, like boosting their sense of fulfillment. But don't stay too long! If your pet starts to hint that they're overwhelmed and they want to go home then it's probably time to wrap up the trip.

You can take them to:
- The park
- A forest
- The aquarium
- The beach

![beachvacation](https://github.com/user-attachments/assets/05876563-14ce-4c40-b7ae-c9dc321d1562)

### Gardening

Through the store you can buy different gardening related items, such as pots, seeds, tools, and fertilizer. Once you've bought some of those things you can then use the gardening menu to place pots around your different rooms, and you can then plant seeds in them (you can also plant seeds directly into the ground outside.)

Once you have a plant started, you should keep them watered over time to keep them growing. And if you fertilize them as well you can really get them to thrive.

Having healthy plants around will give extra boosts to your pet's satisfaction.

### Playdates

You can access the "Social" menu to let your cat go on playdates with other cats! If two Catode32 devices are near each other and both access the social menu, then they'll broadcast availability to each other and you can start a playdate.

Both cats will appear on both devices, and the pets will interact and build social connections. Cats will start to remember friends they've spent a lot of time with.

The devices will also activate their wireless features whenever the cats are in the outside or treehouse scenes, but in a more subtle way. The cats won't see each other directly, but if one vocalizes while outside then any nearby cats who are also outside in their own yards will hear it and they might chatter back.

### Home Comfort

Periodically, your device will use wifi to get a sense of the world around it. It will just do a quick scan to see the names of nearby wireless networks and slowly build up a list of "familiar" ones. Once it has learned what networks you spend the most time around your cat will then feel more comfortable and safe around that location. If you travel to unfamiliar places your cat might be a bit more skittish and less comfortable until they spend some time getting familiar with that new space.

The intent is that a pet left at home is calmer, sleeps better, and plays more freely, while a pet taken somewhere unfamiliar becomes more anxious and restless.

### Sickness

Your pet can become ill if they aren't taken care of. For example, if you feed them too many snacks in a row, leave them outside in the rain/snow, or don't maintain their fullness and cleanliness, then your pet may become increasingly sick.

When they're sick, squiggely lines appear above their head and they will exhibit fewer behaviors. If they're very sick then they might just want to sulk around and rest.

To care for a sick pet and nurture them back to health, make sure they're well fed and groomed, and let them sleep to recover, ideally in the bedroom. You can also buy medicine at the store. If you feed your pet medicine then they'll get a boost to their recovery the next time they rest. Extra medicine doesn't stack, so just give them one dose between naps.

## Controls

- **D-pad**: Navigate / Move camera
- **A**: Select/confirm
- **B**: Back/cancel
- **Menu button 1**: Global menu options (always the same)
- **Menu button 2**: Contextual menu options (based on the current scene)



## Setup

### Hardware Requirements

- **ESP32-C6 SuperMini** OR **ESP32-C3** development board
- **SSD1306 OLED Display** (128x64, I2C)
- **8 Push Buttons** for input

### Software Requirements

- Rust toolchain (install via [rustup](https://rustup.rs))
- `espflash` (`cargo install espflash`)

### Board Configuration

The project supports both ESP32-C6 and ESP32-C3 boards. Board selection is done through Cargo features on the `catode32-firmware` crate. Exactly one of `c6` or `c3` must be active. C6 is the default.

Build and flash for ESP32-C6:
```bash
cargo run
```

Build and flash for ESP32-C3:
```bash
cargo run --no-default-features --features c3 --target riscv32imc-unknown-none-elf
```

The two chips use different RISC-V variants, so the `--target` flag matters. C6 is `riscv32imac-unknown-none-elf` (with the A extension) and C3 is `riscv32imc-unknown-none-elf` (no A extension). The C6 target is set as the default in `.cargo/config.toml`, so you only need to pass `--target` when building for C3.

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
| UP     | GPIO14   |
| DOWN   | GPIO18   |
| LEFT   | GPIO20   |
| RIGHT  | GPIO19   |
| A      | GPIO1    |
| B      | GPIO0    |
| MENU1  | GPIO3    |
| MENU2  | GPIO2    |

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

> **Note:** The ESP32-C3 configuration avoids strapping pins (GPIO2, GPIO8, GPIO9) to prevent boot issues.

> **Warning:** The C3 build currently fails to compile and is untested on hardware. Deep sleep is C6-specific; see the `TODO(c3)` note on `enter_deep_sleep` in `crates/core/src/game.rs`.

## Installation

The firmware is a single Rust binary. There is no separate filesystem or asset upload step. Everything (game logic, sprites, translation strings) is compiled into one image and flashed to the device.

### 1. Set Up Build Tools (one-time)

Install Rust via [rustup](https://rustup.rs) if you don't already have it. The `rust-toolchain.toml` in this repo pins the toolchain and installs the RISC-V targets automatically the first time you build.

Install `espflash`, which builds, flashes, and monitors the device:
```bash
cargo install espflash
```

The `espflash` runner is already wired up in `.cargo/config.toml`, so `cargo run` on the firmware crate will flash and start a serial monitor.

### 2. Build and Flash

For ESP32-C6 (default):
```bash
cargo run --release
```

For ESP32-C3:
```bash
cargo run --release --no-default-features --features c3 --target riscv32imc-unknown-none-elf
```

`espflash` auto-detects the serial port. If you have more than one device connected, pass `--port` after a `--` separator:
```bash
cargo run --release -- --port /dev/tty.usbmodem1234
```

`cargo run` will build, flash, and drop you into the serial monitor. Press `Ctrl+C` to exit the monitor without resetting the device.



## Desktop Emulator

The game can be run on your desktop. That allows you to experiment and play with the project without needing to setup a real ESP32.

Read the [desktop version instructions](https://github.com/moonbench/catode32/wiki/Desktop-Version) for details on how to use the emulator.

[<img width="1024" height="538" alt="screen08" src="https://github.com/user-attachments/assets/bf33c3e3-28e9-4d1a-af36-4f4c7b349724" />](https://github.com/moonbench/catode32/wiki/Desktop-Version)


## Development Workflow

For the fastest iteration, run the game directly on the desktop. See the Desktop Emulator section above for the command that matches your machine (Apple Silicon, Intel Mac, or Linux). The desktop build uses the same `catode32-core` crate that the firmware uses, so most gameplay changes can be tested without touching hardware.

When you need to test on the device itself, use `cargo run` from the workspace root. It builds the firmware crate (the default workspace member), flashes it via `espflash`, and opens a serial monitor:

```bash
cargo run --release
```

Debug builds work too and are faster to compile, but release builds are strongly recommended for on-device testing because the binary is much smaller and the game runs closer to full speed.

## Scripts

[TODO] Add a rust version of the test_hardware script.

## Localization

The project supports several different languages. Specifically:
- English
- Dutch
- Italian
- Spanish
- French
- German

Take a look at the [instructions for building with different languages](https://github.com/moonbench/catode32/wiki/International-Language-Support) for more details.

## Running the Game

Once the firmware is flashed, the game starts automatically on power-up or reset.

To flash a fresh build and drop into the serial monitor:

```bash
cargo run --release
```

To run the desktop emulator instead of flashing hardware, use the command from the Desktop Emulator section for your machine, for example:

```bash
cargo run -p catode32-desktop --target aarch64-apple-darwin
```

## Contributing

It's helpful to open an issue prior to making a PR to allow discussion on the changes.

It's also helpful to keep PRs small and targeted.
