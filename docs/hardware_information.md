# Hardware Information

## Board Overview

This board is a consumer sleep-environment sensing board based on the ESP32-C3-WROOM-02-N4 module.

It collects bedroom environmental data including:

- Temperature
- Relative humidity
- Ambient light
- Relative acoustic noise level

This document only describes the hardware. It does not define firmware framework, software architecture, or upload tools.

---

## Main Components

| Function | Part Number | Interface | Notes |
|---|---|---|---|
| MCU / wireless module | ESP32-C3-WROOM-02-N4 | USB, I²C, ADC, GPIO | ESP32-C3 module with 4 MB flash and PCB antenna |
| Temperature / humidity sensor | SHT40-AD1B-R2 | I²C | Address: `0x44` |
| Ambient light sensor | OPT3001IDNPRQ1 | I²C | ADDR tied to VDD, address: `0x45`; INT not connected |
| Analog MEMS microphone | MSM381ACP003 | Analog output | Output connected to ESP32-C3 ADC input |
| LDO regulator | AP2112K-3.3TRG1 | Power | 5 V to 3.3 V regulator |
| USB ESD protection | USBLC6-2SC6 | USB D+ / D- | Placed near USB-C connector |
| USB input protection | SMD0603-075 PTC | VBUS | In series with USB VBUS input |

---

## Datasheets

| Part | Datasheet URL |
|---|---|
| ESP32-C3-WROOM-02-N4 | https://www.espressif.com/sites/default/files/documentation/esp32-c3-wroom-02_datasheet_en.pdf |
| SHT40-AD1B-R2 / SHT4x series | https://sensirion.com/resource/datasheet/sht4x |
| OPT3001IDNPRQ1 / OPT3001-Q1 | https://www.ti.com/lit/ds/symlink/opt3001-q1.pdf |
| MSM381ACP003 | https://uploadcdn.oneyac.com/attachments/files/brand_pdf/memsensing/27/1E/MSM381ACP003.pdf |
| AP2112K-3.3TRG1 / AP2112 | https://www.diodes.com/datasheet/download/AP2112.pdf |
| USBLC6-2SC6 / USBLC6-2 | https://www.st.com/resource/en/datasheet/usblc6-2.pdf |
| SMD0603-075 PTC | https://www.lcsc.com/datasheet/C70053.pdf |

---

## Power Tree

```text
USB-C VBUS
  → PTC fuse
  → power switch
  → AP2112K-3.3 LDO
  → +3.3V system rail
```

The +3.3 V rail powers:

```text
ESP32-C3-WROOM-02-N4
SHT40
OPT3001
MSM381ACP003 microphone
I²C pull-up resistors
status LEDs
extension headers
```

---

## USB-C Connector

USB-C is used for 5 V power input and native USB connection to the ESP32-C3.

```text
VBUS → PTC → power switch → LDO input
GND  → system GND
CC1  → 5.1kΩ → GND
CC2  → 5.1kΩ → GND
D+   → USB ESD → 0Ω series resistor → ESP32-C3 IO19
D-   → USB ESD → 0Ω series resistor → ESP32-C3 IO18
SBU1 → NC
SBU2 → NC
Shield / EH → GND
```

USB data mapping:

```text
USB D+ = ESP32-C3 IO19
USB D- = ESP32-C3 IO18
```

For a 16-pin USB-C connector with duplicated USB2.0 pins:

```text
A6 + B6 → USB D+
A7 + B7 → USB D-
```

The USBLC6-2SC6 should be placed close to the USB-C connector.

---

## ESP32-C3 Pin Assignment

| Function | ESP32-C3 GPIO | Notes |
|---|---:|---|
| USB D- | IO18 | Native USB D- |
| USB D+ | IO19 | Native USB D+ |
| I²C SDA | IO4 | Shared by SHT40, OPT3001, and I²C expansion header |
| I²C SCL | IO5 | Shared by SHT40, OPT3001, and I²C expansion header |
| Microphone ADC | IO3 / ADC1_CH3 | Analog microphone signal input |
| LED1 | IO0 | Active-low |
| LED2 | IO1 | Active-low |
| UART RX | IO20 | Debug header |
| UART TX | IO21 | Debug header |
| BOOT | IO9 | Active-low boot mode button |
| RESET | EN | Active-low reset |
| Strap pin | IO8 | Pulled up to 3.3 V; not used as normal I/O |

---

## Boot and Reset Circuit

### EN / Reset

```text
3.3V → 10kΩ → EN
EN → 1µF → GND
EN → RESET button → GND
```

EN is active-low. Pulling EN low resets the ESP32-C3.

### BOOT / IO9

```text
3.3V → 10kΩ → IO9
IO9 → BOOT button → GND
```

IO9 is active-low for boot mode selection.

Important hardware note:

```text
No debounce capacitor should be placed on IO9.
```

A capacitor on IO9 can delay the BOOT pin rising during power-up and may cause unreliable startup.

### IO8

```text
IO8 → 10kΩ → 3.3V
```

IO8 is kept high during boot and should not be used for external circuitry that may pull it low during reset or power-up.

---

## I²C Bus

The board has one shared I²C bus:

```text
SDA = IO4
SCL = IO5
```

Pull-up resistors:

```text
SDA → 4.7kΩ → 3.3V
SCL → 4.7kΩ → 3.3V
```

I²C devices:

| Device | Address | Notes |
|---|---:|---|
| SHT40-AD1B-R2 | `0x44` | Temperature / humidity |
| OPT3001IDNPRQ1 | `0x45` | ADDR tied to VDD |

The I²C expansion header is connected to the same bus.

Suggested I²C expansion header:

```text
3V3
GND
SDA / IO4
SCL / IO5
```

Only one set of pull-up resistors is required on the I²C bus.

---

## SHT40 Temperature / Humidity Sensor

Part number:

```text
SHT40-AD1B-R2
```

Connection:

```text
VDD → 3.3V
VSS → GND
SDA → I²C SDA / IO4
SCL → I²C SCL / IO5
```

Decoupling:

```text
100nF between VDD and VSS, placed close to the sensor
```

I²C address:

```text
0x44
```

---

## OPT3001 Ambient Light Sensor

Part number:

```text
OPT3001IDNPRQ1
```

Connection:

```text
VDD  → 3.3V
GND  → GND
SDA  → I²C SDA / IO4
SCL  → I²C SCL / IO5
ADDR → 3.3V
INT  → NC
```

ADDR is tied to VDD to select:

```text
OPT3001 I²C address = 0x45
```

INT is not connected. The device is intended to be read by polling.

Decoupling:

```text
100nF between VDD and GND, placed close to the sensor
```

---

## MSM381ACP003 Analog MEMS Microphone

Part number:

```text
MSM381ACP003
```

Type:

```text
Analog output MEMS microphone
Top-ported
Omnidirectional
```

Power filtering:

```text
3.3V → 22Ω → MIC_VDD
MIC_VDD → 1µF → GND
MIC_VDD → 100nF → GND
```

Signal path:

```text
MIC_OUT → 1kΩ → ADC node / IO3
ADC node → 10nF → GND
```

Ground:

```text
MIC_GND → system GND
```

ADC input:

```text
ESP32-C3 IO3 / ADC1_CH3
```

The microphone output is an analog signal with DC bias. The measured signal should be treated as relative acoustic level unless externally calibrated.

---

## LEDs

Two status LEDs are connected as active-low outputs.

```text
3.3V → resistor → LED anode
LED cathode → GPIO
```

| LED | GPIO | Logic |
|---|---:|---|
| LED1 | IO0 | LOW = on, HIGH = off |
| LED2 | IO1 | LOW = on, HIGH = off |

Recommended resistor value:

```text
2.2kΩ to 4.7kΩ
```

---

## Debug / Expansion Headers

Suggested debug header signals:

```text
3V3
GND
TX / IO21
RX / IO20
EN
BOOT / IO9
```

Suggested I²C expansion header:

```text
3V3
GND
SDA / IO4
SCL / IO5
```

Optional spare GPIOs may be routed to pads or headers, but IO8 and IO9 should be treated as boot-related pins.

---

## Hardware Notes

```text
USB uses ESP32-C3 native USB, not an external USB-UART bridge.
USB D+ / D- should not be used as normal GPIO.
IO18 / IO19 are reserved for USB.
I²C pull-ups are already present on the board.
OPT3001 address is 0x45 because ADDR is tied to VDD.
SHT40 address is 0x44.
OPT3001 INT is not connected.
LEDs are active-low.
The microphone signal is an analog biased signal and requires DC removal in processing.
The microphone path is not calibrated for absolute dB SPL.
IO9 must not have a debounce capacitor.
IO8 is pulled high and should not be externally pulled low during boot.
```
