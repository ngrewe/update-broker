# Update Broker: Using locksmithd on Ubuntu (and other Debian derived distributions)

[locksmithd](https://github.com/coreos/locksmithd) is a very useful tool for coordinating reboots among a fleet of machines. By default, it's use is limited to CoreOS Container Linux. **update-broker** facilitates using locksmithd on Ubuntu or other Debian derived distributions.

update-broker is a small daemon that allows the apt package manager to provide notifications
similar to CoreOS' [update\_engine](https://github.com/coreos/update_engine). It works by monitoring the existence of the file `/var/run/reboot-required` and
notifying locksmithd when it is created.

## Installation

### From Packages

For Ubuntu 16.04 and 18.04 you can install update-broker and locksmithd from packages:

```sh
sudo apt-key adv --keyserver keyserver.ubuntu.com --recv AF0E925C4504784BF4E0FFF0C90E4BD2B36E75B9
echo "deb https://dl.bintray.com/glaux/production $(lsb_release -s -c) main" | sudo tee -a /etc/apt/sources.list.d/locksmithd.list
sudo apt-get update
sudo apt-get install locksmithd
```

### From Source

#### Update Broker

Apart from a [reasonably recent Rust](https://rustup.rs/), Update Broker depends on libdbus and libsystemd.

```sh
curl https://sh.rustup.rs -sSf | sh
sudo apt-get install libsystemd-dev libdbus-1-dev
git clone https://github.com/FutureTVGroup/update-broker.git
cd update-broker
cargo build
sudo cp target/release/update-broker /usr/local/sbin/
sudo cp assets/com.futuretv-group.UpdateBroker.conf /etc/dbus-1/system.d/
cat assets/update-broker.service| sed -e "s%/usr/sbin/%/usr/local/sbin/%" | sudo tee -a /etc/systemd/system/update-broker.service
sudo systemctl enable update-broker
sudo systemctl start update-broker
```

#### Locksmithd

Locksmithd has no dependencies apart from Go.

```sh
sudo apt-get install golang-any
git clone https://github.com/coreos/locksmith
cd locksmith
make
sudo cp bin/locksmithctl /usr/local/sbin/locksmithctl
sudo ln -s /usr/local/sbin/locksmithctl /usr/local/sbin/locksmithd
cat systemd/locksmithd.service| sed -e "s%/usr/lib/locksmith/%/usr/local/sbin/%" | sudo tee -a /etc/systemd/system/locksmithd.service
sudo mkdir -p /etc/coreos/
echo "REBOOT_STRATEGY=off" | sudo tee -a /etc/coreos/update.conf
sudo systemctl enable locksmithd
sudo systemctl start locksmithd
```

## Configuration

There are no differences compared to [configuring locksmithd on CoreOS Container Linux](https://github.com/coreos/locksmith#configuration).
