#!/bin/bash
clear

PROJECT_DIR="/Users/kadir/Desktop/RubriKa/RubrikaV3"
TAURI_DIR="$PROJECT_DIR/src-tauri"
TARGET_DIR="$TAURI_DIR/target"

echo "RubrikaV3 Rust Build Temizleyici"
echo "--------------------------------"
echo
echo "Temizlenecek klasör:"
echo "$TARGET_DIR"
echo

if [ ! -d "$PROJECT_DIR" ]; then
  echo "HATA: RubrikaV3 klasörü bulunamadı:"
  echo "$PROJECT_DIR"
  echo
  read -p "Kapatmak için Enter..."
  exit 1
fi

cd "$PROJECT_DIR" || exit 1

echo "Mevcut boyutlar:"
du -sh "$TARGET_DIR" 2>/dev/null || echo "target klasörü yok."
du -sh "$PROJECT_DIR/node_modules" 2>/dev/null || true
echo

echo "Bu işlem sadece Rust build cache'i temizler."
echo "Kodlar, PDF'ler, project.json, OCR sonuçları ve Documents/RubrikaV3/Projects silinmez."
echo

read -p "src-tauri/target temizlensin mi? [y/N]: " CONFIRM

case "$CONFIRM" in
  y|Y|yes|YES|evet|EVET)
    echo
    echo "Dev süreçleri kapatılıyor..."
    pkill -f "target/debug/app" 2>/dev/null || true
    pkill -f "cargo  run" 2>/dev/null || true
    pkill -f "vite" 2>/dev/null || true

    echo
    echo "Rust build cache temizleniyor..."
    cargo clean --manifest-path "$TAURI_DIR/Cargo.toml"

    echo
    echo "Temizlik tamamlandı."
    echo
    echo "Yeni boyut:"
    du -sh "$TARGET_DIR" 2>/dev/null || echo "target klasörü temizlendi."
    ;;
  *)
    echo
    echo "İşlem iptal edildi."
    ;;
esac

echo
echo "Uygulamayı tekrar açmak için:"
echo "cd $PROJECT_DIR"
echo "npm run tauri:dev"
echo
read -p "Kapatmak için Enter..."
