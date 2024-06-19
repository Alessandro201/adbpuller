# AdbPuller
Copy files from Android folders using ADB drivers


## Features
- Copy files from Android **skipping already copied** ones
- Copy **presets** for media, whatsapp media, and whatsapp backups
- Execute a **dry run** to check which files will be copied and where
- Copy **metadata** like `last modification date` by default
- Skip filepaths from a given file (one filepath per line)

#### Planned
- Exclude files based on regex
- Inlcude files based on regex

## Usage
```
Pull files from android using ADB drivers

Usage: adb_puller [OPTIONS] <--sources [<SOURCES>...]|--copy-media|--copy-whatsapp|--copy-whatsapp-backups>

Options:
  -s, --sources [<SOURCES>...]  The folder(s) or item(s) to copy
  -m, --copy-media              Add /sdcard/DCIM and /sdcard/Pictures to the sources
  -w, --copy-whatsapp           Add Whatsapp Audio, Images, Video and Voice Notes to the sources
  -b, --copy-whatsapp-backups   Add Whatsapp Backup and Databases folders to the sources
  -d, --dest <DEST>             The folder in which to copy the files [default: .]
      --skip [<SKIP>...]        Skip files written in a file
      --dry-run                 Print which files would be copied and where
  -f, --force                   Overwrite files already present in the destination folder
      --no-metadata             Don't copy metadata such as last modification date ecc..
  -h, --help                    Print help (see more with '--help')
  -V, --version                 Print version
```
<br>

Copy directories using the preset to copy media in the current directory:
```
adb_puller -m
```

Copy directories using the preset to copy whatsapp media and whatsapp backups in the specified folder without:
```
adb_puller -w -b -d ./Whatsapp
```

Copy directories from `/sdcard/Downloads` into `./AndroidDownloads` overwriting any existing file:
```
adb_puller -s /sdcard/Downloads --force -d ./AndroidDownloads
```


#### Presets 
- `--copy-media` will copy files from Media directories:
  ```
  /sdcard/DCIM
  /sdcard/Pictures
  ```

- `--copy-whatsapp` will copy files from Whatsapp Media directories:
  ```
  /sdcard/Android/media/com.whatsapp/WhatsApp/Media/WhatsApp Audio
  /sdcard/Android/media/com.whatsapp/WhatsApp/Media/WhatsApp Images
  /sdcard/Android/media/com.whatsapp/WhatsApp/Media/WhatsApp Video
  /sdcard/Android/media/com.whatsapp/WhatsApp/Media/WhatsApp Voice Notes
  /sdcard/Android/media/com.whatsapp/WhatsApp/Media/WhatsApp Video Notes
  /sdcard/Android/media/com.whatsapp/WhatsApp/Media/WhatsApp Documents
  ```

- `--copy-whatsapp-backups` will copy the backups of Whatsapp:
  ```
  /sdcard/Android/media/com.whatsapp/WhatsApp/Backups
  /sdcard/Android/media/com.whatsapp/WhatsApp/Databases
  ```


## Installation
You can download the latest binary from the release page which comes with the ADB drivers, and skip to the [Setup](`target/release/adbpuller`) section. Alternatively you can build your binary from source.


### Build from source
You need to have:
- [RUST](https://www.rust-lang.org/tools/install) installed.
- ADB drivers. `adbpuller` will first try to find the `adb` binary in the same folder, then it will search in the `$PATH`. To install them you can either:

  - ***[Recommended on Linux]*** Install ADB drivers from a package manager like `apt`:
    ```bash
    sudo apt install adb
    ```
  
  - Or download the drivers from Google and unzip them:

    On Ubuntu:
    ```bash
    curl -O https://dl.google.com/android/repository/platform-tools-latest-linux.zip
    unzip platform-tools-latest-linux.zip
    ```
    A folder called `platform-tools` will be created.
    
    On Windows:
    Download the [ADB driver for windows](https://dl.google.com/android/repository/platform-tools-latest-windows.zip) and extract them in a folder. 

<br>

Then clone the repository, move into the directory and build the binary.

``` bash
git clone https://github.com/Alessandro201/adbpuller.git
cd adbpuller
cargo build -r
```

The binary will be at `target/release/adbpuller`. 
If you downloaded the drivers manually, either add the `platform-tools` folder in your `$PATH` or place the ``target/release/adbpuller`` binary in the same folder. In the latter case your folder should look like this:
```
platform-tools/
├── lib64/ 
│   └── ...
├── adb
├── adbpuller
├── etc1tool
├── fastboot
├── hprof-conv
├── make_f2fs
├── make_f2fs_casefold
├── mke2fs
├── mke2fs.conf
├── NOTICE.txt
├── source.properties
└── sqlite3
```
After moving the adbpuller binary you are free to delete the `adbpuller/` directory you downloaded with `git`.


## Setup
You need to enable `Debug USB` on you Android, [here](https://www.xda-developers.com/install-adb-windows-macos-linux/) is a guide from XDA Deevelopers on how to do it.
Then after connecting your android to the pc via a cable, run the following command to establish a connection between them. 
```
adb devices
```

Now accept the popup on the phone and ***rerun*** the command again. The output should look like this:
```
List of devices attached
12ec6c18        device
```

Now you are ready!
