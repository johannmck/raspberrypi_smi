# Raspberry Pi SMI interface
  
The Raspberry Pi features a set of General-Purpose Input/Output (GPIO) pins that serve as crucial interfaces for connecting external devices. These GPIO pins facilitate bidirectional communication, allowing the Raspberry Pi to interact with a diverse range of peripherals. Programmable to support various protocols, these pins enable the implementation of specific functionalities. Supported communication protocols include GPIO for basic digital control, I2C for serial communication with sensors, SPI for high-speed data transfer between microcontrollers and peripherals, and UART for serial communication with devices like GPS modules and Bluetooth modules.

The Raspberry Pi features a Secondary Memory Interface (SMI) as part of its peripheral set. The Secondary Memory Interface is a hardware feature designed to facilitate communication between the Broadcom system-on-chip (SoC) and external memory devices, such as additional RAM or non-volatile memory (e.g., Flash memory).

The aim of this project is to output and read data on this interface, and will cover direct and DMA data transfer to the outputs, which requires allocating uncached memory via the VideoCore mailbox (using ioctl).

### References

[1] This project heaviliy cites and follows the path of this excellent Lean2 blog post https://iosoft.blog/2020/07/16/raspberry-pi-smi/ , but with some expansion in areas which were not obvious to me and with additional sources and references.

[2] Descriptions of functionality are provided in the partial BCM 2835 datasheet https://elinux.org/BCM2835_datasheet_errata and errata https://elinux.org/BCM2835_datasheet_errata .

[3] For the SMI interface there is a compilation of datasheet information from G.J. Van Loo (2017) https://www.eevblog.com/forum/projects/state-of-raspberry-pi-adc-support-in-2023/?action=dlattach;attach=1855891

## Hardware connections
The relevant pins for the SMI interface are available as ALT1 function and labeled as SD[X] (data bus) , SA[X] (address bus) , SOE_N / SE (control) , SWE_N / SRW_N (control).
![Raspberry Pi 40 pin header GPIO schematic](https://www.raspberrypi.org/app/uploads/2014/04/bplus-gpio.png)
Full table of pin mapping including ALT functions [https://elinux.org/RPi_BCM2835_GPIOs](https://elinux.org/RPi_BCM2835_GPIOs)
| Header Pin | GPIO   | ALT1 function   |
|------------|--------|------------------|
| 3          | GPIO2  | SA3              |
| 5          | GPIO3  | SA2              |
| 7          | GPIO4  | SA1              |
| 8          | GPIO14 | SD6              |
| 10         | GPIO15 | SD7              |
| 11         | GPIO17 | SD9              |
| 12         | GPIO18 | SD10             |
| 13         | GPIO27 | TE1              |
| 15         | GPIO22 | SD14             |
| 16         | GPIO23 | SD15             |
| 18         | GPIO24 | SD16             |
| 19         | GPIO10 | SD2              |
| 21         | GPIO9  | SD1              |
| 22         | GPIO25 | SD17             |
| 23         | GPIO11 | SD3              |
| 24         | GPIO8  | SD0              |
| 26         | GPIO7  | SWE_N / SRW_N    |
| 27         | GPIO0  | SA5              |
| 28         | GPIO1  | SA4              |
| 29         | GPIO5  | SA0              |
| 31         | GPIO6  | SOE_N / SE       |
| 32         | GPIO12 | SD4              |
| 33         | GPIO13 | SD5              |
| 35         | GPIO19 | SD11             |
| 36         | GPIO16 | SD8              |
| 37         | GPIO26 | TE0              |
| 38         | GPIO20 | SD12             |
| 40         | GPIO21 | SD13             |


https://github.com/Ysurac/raspberry_kernel_mptcp/blob/master/include/linux/broadcom/bcm2835_smi.h

https://www.eevblog.com/forum/projects/state-of-raspberry-pi-adc-support-in-2023/?action=dlattach;attach=1855891

## SMI interface overview
A compilation of relevant registers and bitfields are in a later section. We can see from the register definitions below that the read transfer width can be 8-bit, 16-bit, 18-bit or 9-bit and the address bus is 6-bit.

-  **Address Bus SA[X]:**
    
    -   The address bus is used to specify a memory address or I/O port address. It indicates the location in memory or I/O space where data should be read from or written to.
-  **Data Bus SD[X]:**
    
    -   The data bus is responsible for carrying the actual data between the microprocessor and memory or I/O devices. It is bidirectional, allowing data to flow in both directions.
-  **Read Strobe (Read Signal) SOE_N/SE:**
    
    -   The read strobe is a signal indicating that the microprocessor is requesting data from the specified address. When this signal is asserted, the addressed memory or I/O device is expected to place data on the data bus.
- **Write Strobe (Write Signal) SWE_N/SRW_N:**
    
    -   The write strobe is a signal indicating that the microprocessor is sending data to the specified address. When this signal is asserted, the addressed memory or I/O device should read the data from the data bus and store it at the specified address.

> The "_N" in the signal names typically denotes that these are active-low signals, meaning they are considered active when brought low.

### Timing
```
	 /* Timing for reads (writes the same but for WE)
	 *
	 * OE ----------+	   +--------------------
	 *		|	   |
	 *              +----------+
	 * SD -<==============================>-----------
	 * SA -<=========================================>-
	 *    <-setup->  <-strobe ->  <-hold ->  <- pace ->
	 */
```
  
-  *Setup Time (Setup):*
The time interval between the assertion of Chip Select (CS) or Address and the initiation of the actual data transfer. It allows the target device to stabilize and prepare for the incoming data. It's essential for ensuring that the data lines are stable before the actual transfer begins.
- *Strobe Time (Strobe):*
The duration for which the Read or Write Strobe signal is asserted. Strobe indicates the period during which the actual data is being transferred. For a read operation, it's the time the target device should make its data available on the bus. For a write operation, it's the time when the source device drives the data onto the bus.
- *Hold Time (Hold):*
The time interval between the completion of the data transfer and the de-assertion of Chip Select (CS) or Address. It provides a margin for the target device to finish processing the received data before the CS or Address signal is de-asserted. This ensures that the target device is not prematurely interrupted.
- *Pace Time (Pace):*
The time interval between consecutive data transfers. It establishes the timing between successive transfers. It ensures that there's a sufficient gap between data transfers to accommodate the specific requirements of the devices involved.

## SMI direct read / write
### Generic functionality and setup
First some generic functionality as provided in the Lean2 blog, starting with a C macro to help define the registers in a convenient to access format.

```c
#include <stdint.h>

#define REG_32(name, fields) \
    typedef union { \
        struct { \
            volatile uint32_t fields; \
        }; \
        volatile uint32_t value; \
    } name
```

With this macro we are creating a union type that represents a 32-bit register. The individual bits or fields within the register are accessed through the structure members, and the entire 32-bit register is accessed through the `value` member. The use of `volatile` is important in embedded systems programming to ensure that the compiler doesn't optimize away or reorder read and write operations on the register.

An example of this for `CM_SMI_CTL` register is 

```c
// Define CM_SMI_CTL register
REG_32(CM_SMI_CTL, {
       uint32_t  CM_SMI_CTL_FLIP : 1;
       uint32_t  CM_SMI_CTL_BUSY : 1;
       uint32_t  CM_SMI_CTL_KILL : 1;
       uint32_t  CM_SMI_CTL_ENAB : 1;
       uint32_t  CM_SMI_CTL_SRC : 4;
});
```


### Direct write


## BCM register map
The Raspberry Pi 3B has a physical base address for peripherals `BCM_PERI_BASE` of `0x3F000000`.
The SMI registers base address is defined as `((BCM_PERI_BASE) + 0x600000)`.

Following is a summary of the SMI clock manager registers and bitfields:

| Register Name  | Address Offset | Description                                     |
|----------------|-----------------|-------------------------------------------------|
| CM_SMI_CTL     | 0x00            | Control Register                               |
| CM_SMI_DIV     | 0x04            | Divider Register                               |


| Bitfield         | Bit Range | Description                                     |
|------------------|-----------|-------------------------------------------------|
| CM_SMI_CTL_FLIP  | 8         | Flip the data on the SMI data line              |
| CM_SMI_CTL_BUSY  | 7         | SMI busy indicator                              |
| CM_SMI_CTL_KILL  | 5         | Kill the SMI clock                              |
| CM_SMI_CTL_ENAB  | 4         | Enable the SMI clock                            |
| CM_SMI_CTL_SRC   | 0:3       | Source of the SMI clock, configurable bits     |

| Bitfield             | Bit Range | Description                       |
|----------------------|-----------|-----------------------------------|
| CM_SMI_DIV_DIVI      | 12:15     | Integer part of the divisor       |
| CM_SMI_DIV_DIVF      | 4:11      | Fractional part of the divisor    |


Following is a summary of the SMI registers and bitfields:

| Register Name | Address Offset | Description                                     |
|---------------|-----------------|-------------------------------------------------|
| SMICS         | 0x00            | Control + Status Register                       |
| SMIL          | 0x04            | Length/Count (Number of External Transfers)     |
| SMIA          | 0x08            | Address Register                                |
| SMID          | 0x0C            | Data Register                                   |
| SMIDSR0       | 0x10            | Device 0 Read Settings                          |
| SMIDSW0       | 0x14            | Device 0 Write Settings                         |
| SMIDSR1       | 0x18            | Device 1 Read Settings                          |
| SMIDSW1       | 0x1C            | Device 1 Write Settings                         |
| SMIDSR2       | 0x20            | Device 2 Read Settings                          |
| SMIDSW2       | 0x24            | Device 2 Write Settings                         |
| SMIDSR3       | 0x28            | Device 3 Read Settings                          |
| SMIDSW3       | 0x2C            | Device 3 Write Settings                         |
| SMIDC         | 0x30            | DMA Control Registers                           |
| SMIDCS        | 0x34            | Direct Control/Status Register                  |
| SMIDA         | 0x38            | Direct Address Register                         |
| SMIDD         | 0x3C            | Direct Data Registers                           |
| SMIFD         | 0x40            | FIFO Debug Register                             |


| Bitfield      | Bit   | Description                                       |
|---------------|-------|---------------------------------------------------|
| SMICS_RXF     | 31    | RX fifo full: 1 when RX fifo is full              |
| SMICS_TXE     | 30    | TX fifo empty: 1 when empty                       |
| SMICS_RXD     | 29    | RX fifo contains data: 1 when there is data       |
| SMICS_TXD     | 28    | TX fifo can accept data: 1 when true              |
| SMICS_RXR     | 27    | RX fifo needs reading: 1 when more than 3/4 full, or when "DONE" and fifo not emptied |
| SMICS_TXW     | 26    | TX fifo needs writing: 1 when less than 1/4 full  |
| SMICS_AFERR   | 25    | AXI FIFO error: 1 when fifo read when empty or written when full. Write 1 to clear |
| SMICS_EDREQ   | 15    | 1 when external DREQ received                     |
| SMICS_PXLDAT  | 14    | Pixel data: write 1 to enable pixel transfer modes. The data in the FIFO‘s will be appropriately packed to suit the pixel format selected (SMIDS[R/W]_[R/W]WIDTH). |
| SMICS_SETERR  | 13    | 1 if there was an error writing to setup regs. Write 1 to clear |
| SMICS_PVMODE  | 12    | Set to 1 to enable pixel valve mode                |
| SMICS_INTR    | 11    | Set to 1 to enable interrupt on RX                 |
| SMICS_INTT    | 10    | Set to 1 to enable interrupt on TX                 |
| SMICS_INTD    | 9     | Set to 1 to enable interrupt on DONE condition    |
| SMICS_TEEN    | 8     | Tear effect mode enabled: Programmed transfers will wait for a TE trigger before writing |
| SMICS_PAD1    | 7     | Padding settings for external transfers: For writes, the number of bytes initially written to the TX fifo that should be ignored. For reads, the number of bytes that will be read before the data and should be dropped |
| SMICS_PAD0    | 6     | Padding settings for external transfers: For writes, the number of bytes initially written to the TX fifo that should be ignored. For reads, the number of bytes that will be read before the data and should be dropped |
| SMICS_WRITE   | 5     | Transfer direction: 1 = write to external device, 0 = read |
| SMICS_CLEAR   | 4     | Write 1 to clear the FIFOs                         |
| SMICS_START   | 3     | Write 1 to start the programmed transfer          |
| SMICS_ACTIVE  | 2     | Reads as 1 when a programmed transfer is underway |
| SMICS_DONE    | 1     | Reads as 1 when transfer finished. For RX, not set until FIFO emptied |
| SMICS_ENABLE  | 0     | Set to 1 to enable the SMI peripheral, 0 to disable |

| Bitfield      | Bit   | Description                                       |
|---------------|-------|---------------------------------------------------|
| SMIA_DEVICE   | 8:9   | Device Select                       |
| SMIA_ADDR     | 0:5   | Address                              |

| Bitfield      | Bit   | Description                                           |
|---------------|-------|-------------------------------------------------------|
| SMIDC_DMAEN   | 28    | DMA enable: Set to 1 to issue DMA requests.           |
| SMIDC_DMAP    | 24    | DMA passthrough: Set to 0 for normal, 1 for DREQ pins.|
| SMIDC_PANICR  | 18:23 | Panic threshold for DMA read.                         |
| SMIDC_PANICW  | 12:17 | Panic threshold for DMA write.                        |
| SMIDC_REQR    | 6:11  | DREQ threshold for DMA read.                          |
| SMIDC_REQW    | 0:5   | DREQ threshold for DMA write.                         |

| Bitfield         | Bit   | Description                                             |
|------------------|-------|---------------------------------------------------------|
| SMIDSR_RWIDTH    | 30:31 | Read transfer width: 00 = 8-bit, 01 = 16-bit, 10 = 18-bit, 11 = 9-bit.|
| SMIDSR_RSETUP    | 24:29 | Read setup time: Number of core cycles between CS/address and read strobe. Min 1, max 64.|
| SMIDSR_MODE68    | 23    | 1 for System 68 mode (enable + direction pins instead of OE + WE pin).|
| SMIDSR_FSETUP    | 22    | If set to 1, setup time only applies to the first transfer after an address change.|
| SMIDSR_RHOLD     | 16:21 | Number of core cycles between read strobe going inactive and CS/address going inactive. Min 1, max 64.|
| SMIDSR_RPACEALL  | 15    | When set to 1, this device's RPACE value will always be used for the next transaction, even if it is not to this device.|
| SMIDSR_RPACE     | 8:14  | Number of core cycles spent waiting between CS deassert and start of the next transfer. Min 1, max 128.|
| SMIDSR_RDREQ     | 7     | 1 = Use external DMA request on SD16 to pace reads from the device. Must also set DMAP in SMICS.|
| SMIDSR_RSTROBE   | 0:6   | Number of cycles to assert the read strobe. Min 1, max 128.|


| Bitfield       | Bit   | Description                                        |
|----------------|-------|----------------------------------------------------|
| SMIDSW_WWIDTH  | 30:31 | Write transfer width. 00 = 8bit, 01 = 16bit, 10= 18bit, 11 = 9bit. |
| SMIDSW_WSETUP  | 24:29 | Number of cycles between CS assert and write strobe. Min 1, max 64. |
| SMIDSW_WFORMAT | 23    | Pixel format of input. 0 = 16bit RGB 565, 1 = 32bit RGBA 8888. |
| SMIDSW_WSWAP   | 22    | 1 = swap pixel data bits. (Use with SMICS_PXLDAT)  |
| SMIDSW_WHOLD   | 16:21 | Time between WE deassert and CS deassert. 1 to 64. |
| SMIDSW_WPACEALL| 15    | 1: this device's WPACE will be used for the next transfer, regardless of that transfer's device. |
| SMIDSW_WPACE   | 8:14  | Cycles between CS deassert and next CS assert. Min 1, max 128. |
| SMIDSW_WDREQ   | 7     | Use external DREQ on pin 17 to pace writes. DMAP must be set in SMICS. |
| SMIDSW_WSTROBE | 0:6   | Number of cycles to assert the write strobe. Min 1, max 128. |

| Bitfield      | Bit | Description                                        |
|---------------|-----|----------------------------------------------------|
| SMIDCS_WRITE  | 3   | Direction of transfer: 1 -> write, 0 -> read       |
| SMIDCS_DONE   | 2   | 1 when a transfer has finished. Write 1 to clear. |
| SMIDCS_START  | 1   | Write 1 to start a transfer, if one is not already underway. |
| SMIDCS_ENABLE | 0   | Write 1 to enable SMI in direct mode.              |

| Bitfield        | Bit   | Description                                               |
|-----------------|-------|-----------------------------------------------------------|
| SMIDA_DEVICE    | 8:9   | Indicates which of the device settings banks should be used. |
| SMIDA_ADDR      | 0:5   | The value to be asserted on the address pins.              |

| Bitfield       | Bit   | Description                                                |
|----------------|-------|------------------------------------------------------------|
| SMIFD_FLVL     | 8:13  | The high-tide mark of FIFO count during the most recent transfer. |
| SMIFD_FCNT     | 0:5   | The current FIFO count.                                    |
