#!/bin/bash
clear

# Betiğin bulunduğu dizini otomatik tespit et (herhangi bir klasörde/bilgisayarda çalışmasını sağlar)
SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"
cd "$SCRIPT_DIR" || exit 1

echo "=========================================="
echo "      Cargo / Rust Build Temizleyici"
echo "=========================================="
echo
echo "Çalışma Dizini: $SCRIPT_DIR"
echo

# Rust projesinin direkt mevcut dizinde mi yoksa src-tauri içinde mi olduğunu kontrol et
if [ -f "Cargo.toml" ]; then
    MANIFEST_PATH="Cargo.toml"
    TARGET_DIR="target"
elif [ -f "src-tauri/Cargo.toml" ]; then
    MANIFEST_PATH="src-tauri/Cargo.toml"
    TARGET_DIR="src-tauri/target"
else
    echo "HATA: Bu klasörde veya src-tauri alt klasöründe Cargo.toml bulunamadı!"
    echo "Lütfen bu dosyayı bir Rust veya Tauri projesinin kök dizinine yerleştirin."
    echo
    read -p "Kapatmak için Enter'a basın..."
    exit 1
fi

echo "Temizlenecek hedef dizin: $TARGET_DIR"
echo

if [ -d "$TARGET_DIR" ]; then
    echo "Mevcut target klasör boyutu:"
    du -sh "$TARGET_DIR" 2>/dev/null
else
    echo "target klasörü henüz oluşmamış veya zaten temiz."
fi
echo

echo "Bu işlem Rust derleme (build) önbelleğini temizler."
echo "Kaynak kodlarınız ve diğer dosyalarınız etkilenmez."
echo

read -p "Temizlik işlemini onaylıyor musunuz? [y/N]: " CONFIRM

case "$CONFIRM" in
  y|Y|yes|YES|evet|EVET)
    echo
    echo "Rust build cache temizleniyor (cargo clean)..."
    cargo clean --manifest-path "$MANIFEST_PATH"

    echo
    echo "✅ Temizlik başarıyla tamamlandı!"
    ;;
  *)
    echo
    echo "İşlem iptal edildi."
    ;;
esac

echo
read -p "Kapatmak için Enter'a basın..."
