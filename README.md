# Funpack S3#1 Temperature Sensor

![UI](./image/ui.png)

Prebuilt image and Linux UI are available in the release page.

## Build

### MCU

Notice: You need to flash a bootloader (Bluetooth AppLoader OTA DFU) first.

Import the project into SimplicityStudio: Project -> Import -> MCU Project.

![Import project](./image/import.png)

Then build and flush the application: Right Click -> Run As -> Silicon Labs ARM Program

![Run](./image/run.png)

### UI

Build and run the ui:

``` shell
cd ui
RUST_LOG=info cargo run
```

