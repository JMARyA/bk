# Maintainer: JMARyA <jmarya@hydrar.de>
pkgname=bk
pkgver=2025.01.07_2e7d4aa
pkgrel=1
pkgdesc="backup utility"
arch=('x86_64')
url="https://git.hydrar.de/jmarya/bk"
license=("MIT")
depends=("borg" "rsync")
makedepends=("rustup" "git")
source=("${pkgname}::git+https://git.hydrar.de/jmarya/bk.git")
sha256sums=("SKIP")

pkgver() {
    cd "$srcdir/$pkgname"
   	echo "$(date +%Y.%m.%d)_$(git rev-parse --short HEAD)"
}

prepare() {
    cd "$srcdir/$pkgname"
    rustup default nightly
    cargo fetch
}

build() {
    cd "$srcdir/$pkgname"
    cargo build --release
}

check() {
    cd "$srcdir/$pkgname"
    cargo test --release
}

package() {
    cd "$srcdir/$pkgname"
    install -Dm755 "target/release/bk" "$pkgdir/usr/bin/bk"
    install -Dm644 "src/systemd/bk.service" "/usr/lib/systemd/system/bk.service"
    install -Dm644 "src/systemd/bk.timer" "/usr/lib/systemd/system/bk.timer"
    install -Dm644 "config.toml" "/etc/bk.toml"
}
