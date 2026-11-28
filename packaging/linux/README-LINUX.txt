ipabazooka @VERSION@ for Linux: install apps on your iPhone or iPad

NEW TO SIDELOADING?
  Open the app and click "? Help" at the top, next to Theme. It explains
  what sideloading is, how to get an app file (.ipa), and how to use the
  app step by step.
====================================================================

WHICH DOWNLOAD SHOULD I USE?
  Debian, Ubuntu, Linux Mint, Pop!_OS, Zorin, elementary:
      sudo apt install ./ipabazooka_@VERSION@_amd64.deb
  Fedora, Nobara:
      sudo dnf install ./ipabazooka-@VERSION@-1.x86_64.rpm
  RHEL, AlmaLinux, Rocky 10 (usbmuxd comes from EPEL):
      sudo dnf install epel-release
      sudo dnf install ./ipabazooka-@VERSION@-1.x86_64.rpm
    (Version 9 doesn't package usbmuxd at all; the app installs and runs,
    but you'd need to build usbmuxd yourself to connect a phone.)
  openSUSE:
      sudo zypper install --allow-unsigned-rpm ./ipabazooka-@VERSION@-1.x86_64.rpm
  Arch, Manjaro, EndeavourOS, CachyOS, Garuda:
      sudo pacman -U ./ipabazooka-@VERSION@-1-x86_64.pkg.tar.zst
  Any other distribution, or no installing at all:
      the AppImage (one file, see below) or this portable folder.

  The .deb, .rpm and Arch packages also install usbmuxd for you and add
  ipabazooka to your app menu.

THE APPIMAGE (one file, nothing to install)
  chmod +x ipabazooka-@VERSION@-x86_64.AppImage
  ./ipabazooka-@VERSION@-x86_64.AppImage
  (You can also right-click it > Properties > "Allow executing as program",
  then double-click it.) You still need usbmuxd, see below.

THIS PORTABLE FOLDER
  Run it straight from here:      ./ipabazooka
  Add it to your app menu (just for you, no password needed):
                                  ./install.sh
  Or for everyone on this PC:     sudo ./install.sh --system
  Remove it again:                ./uninstall.sh   (or sudo ./uninstall.sh --system)

YOU ALSO NEED usbmuxd
  usbmuxd is the small system service that lets Linux talk to an iPhone or
  iPad over USB (the Linux version of what iTunes provides on Windows).
  The .deb, .rpm and Arch packages install it automatically. Otherwise:
      Debian / Ubuntu / Mint:    sudo apt install usbmuxd
      Fedora:                    sudo dnf install usbmuxd
      RHEL / Alma / Rocky 10:    sudo dnf install epel-release
                                 sudo dnf install usbmuxd
      openSUSE:                  sudo zypper install usbmuxd
      Arch / Manjaro:            sudo pacman -S usbmuxd
      Void:                      sudo xbps-install usbmuxd
      NixOS:                     services.usbmuxd.enable = true;
  Linux starts usbmuxd by itself when you plug in a phone.

FIRST RUN
  * Plug in the iPhone with a cable, unlock it and tap "Trust".
    If the Trust prompt never appears, run:  idevicepair pair
    (from the libimobiledevice-utils / libimobiledevice package).
  * Your theme, logs and sign-in data are kept in ~/.config/ipabazooka
    (private to your user account).
  * Apple sign-in data ("anisette") comes from online sign-in servers on
    Linux. If Apple says your password is wrong when it isn't, open
    Advanced options and press "Reset sign-in data", or pick another server.
  * The first install on a new computer may ask you to pick an old signing
    certificate to revoke. That's normal for free Apple IDs.

KEYS
  Alt+S   Start (when an app and a device are chosen)
  Alt+.   Show or hide the status history (the full log)
  Alt+V   Show or hide Advanced options
  F11     Full screen (Esc leaves it)

REQUIREMENTS
  64-bit (x86_64) Linux with glibc 2.28 or newer (2018 and later: Debian 10+,
  Ubuntu 20.04+, Fedora 29+, RHEL/Alma/Rocky 8+, openSUSE Leap 15+, Arch),
  and a graphical desktop (X11 or Wayland). Alpine and other musl-based
  systems are not supported.

More help: https://github.com/Sky3rshin/ipabazooka
