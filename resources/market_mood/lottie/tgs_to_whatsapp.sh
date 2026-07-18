#!/usr/bin/env bash
set -euo pipefail

# Bash's printf obeys LC_NUMERIC. Under pt_BR, a Python-produced value such as
# "20.0" is otherwise rejected because the locale expects "20,0".
export LC_ALL=C
export LANG=C

usage() {
  cat <<'EOF'
Uso:
  tgs_to_whatsapp.sh arquivo.tgs [saida.webp] [qualidade]

Exemplos:
  ./tgs_to_whatsapp.sh animacao.tgs
  ./tgs_to_whatsapp.sh animacao.tgs animacao.webp 75

Dependencias no CachyOS/Arch:
  sudo pacman -S --needed python ffmpeg cairo pango

Na primeira execucao, o script cria um ambiente Python isolado em:
  ~/.cache/tgs-to-whatsapp-venv
EOF
}

[[ $# -ge 1 ]] || { usage; exit 2; }
INPUT=$1
OUTPUT=${2:-"${INPUT%.tgs}.webp"}
QUALITY=${3:-75}

[[ -f "$INPUT" ]] || { echo "Arquivo inexistente: $INPUT" >&2; exit 1; }
[[ "$QUALITY" =~ ^[0-9]+$ ]] && (( QUALITY >= 1 && QUALITY <= 100 )) || {
  echo "Qualidade deve estar entre 1 e 100." >&2
  exit 2
}

PYTHON=${PYTHON:-}
if [[ -z "$PYTHON" ]]; then
  if command -v python3 >/dev/null 2>&1; then PYTHON=python3
  elif command -v python >/dev/null 2>&1; then PYTHON=python
  else echo "Instale Python: sudo pacman -S python" >&2; exit 1
  fi
fi
command -v ffmpeg >/dev/null 2>&1 || {
  echo "Instale ffmpeg: sudo pacman -S ffmpeg" >&2
  exit 1
}
ffmpeg -hide_banner -encoders 2>/dev/null | grep 'libwebp_anim' >/dev/null || {
  echo "Seu ffmpeg nao oferece o encoder libwebp_anim." >&2
  exit 1
}

VENV=${TGS2WEBP_VENV:-"$HOME/.cache/tgs-to-whatsapp-venv"}
if [[ ! -x "$VENV/bin/lottie_convert.py" ]]; then
  echo "Preparando renderizador Lottie em $VENV ..." >&2
  rm -rf "$VENV"
  "$PYTHON" -m venv "$VENV"
  PIP_NO_CACHE_DIR=1 "$VENV/bin/python" -m pip install --upgrade pip >/dev/null
  PIP_NO_CACHE_DIR=1 "$VENV/bin/pip" install \
    'lottie==0.7.2' 'cairosvg>=2.7' 'pillow>=10' >/dev/null
fi
LOTTIE="$VENV/bin/lottie_convert.py"

# Le metadados e faz uma verificacao estrutural que o tgs_check.py nao cobre:
# keyframes interpolados precisam ter as curvas temporais i/o. A ausencia delas
# pode derrubar versoes do rlottie em vez de produzir uma mensagem de erro.
META=$(
  "$PYTHON" - "$INPUT" <<'PY'
import gzip
import json
import sys

path = sys.argv[1]
try:
    with gzip.open(path, "rt", encoding="utf-8") as stream:
        data = json.load(stream)
except (OSError, UnicodeError, json.JSONDecodeError) as exc:
    raise SystemExit(f"TGS invalido: {exc}")

errors = []

def walk(value, path=""):
    if isinstance(value, dict):
        frames = value.get("k")
        if value.get("a") == 1 and isinstance(frames, list):
            for index, keyframe in enumerate(frames[:-1]):
                if not isinstance(keyframe, dict) or keyframe.get("h") == 1:
                    continue
                if "i" not in keyframe or "o" not in keyframe:
                    errors.append(f"{path}/k[{index}]: keyframe interpolado sem i/o")
        for key, child in value.items():
            walk(child, f"{path}/{key}")
    elif isinstance(value, list):
        for index, child in enumerate(value):
            walk(child, f"{path}[{index}]")

walk(data)
if errors:
    print("TGS potencialmente incompativel com rlottie:", file=sys.stderr)
    for error in errors[:20]:
        print(f"  {error}", file=sys.stderr)
    raise SystemExit(4)

fps = float(data.get("fr", 60))
ip = float(data.get("ip", 0))
op = float(data.get("op", ip + 180))
if fps <= 0 or op <= ip:
    raise SystemExit("Metadados de tempo invalidos no TGS")

# TSV evita qualquer ambiguidade de separacao.
print(f"{fps:.12g}\t{int(round(ip))}\t{int(round(op))}\t{(op-ip)/fps:.12g}")
PY
)
IFS=$'\t' read -r SRC_FPS FIRST_FRAME LAST_FRAME DURATION <<< "$META"

TARGET_FPS=${TGS2WEBP_FPS:-20}
MIN_QUALITY=${TGS2WEBP_MIN_QUALITY:-35}
[[ "$TARGET_FPS" =~ ^[0-9]+$ ]] && (( TARGET_FPS >= 1 && TARGET_FPS <= 60 )) || {
  echo "TGS2WEBP_FPS deve ser um inteiro entre 1 e 60." >&2
  exit 2
}
[[ "$MIN_QUALITY" =~ ^[0-9]+$ ]] && (( MIN_QUALITY >= 1 && MIN_QUALITY <= QUALITY )) || {
  echo "TGS2WEBP_MIN_QUALITY deve estar entre 1 e a qualidade inicial." >&2
  exit 2
}

TMP=$(mktemp -d -t tgs2webp.XXXXXX)
trap 'rm -rf "$TMP"' EXIT
GIF="$TMP/render.gif"

calculate_timing() {
  local requested_fps=$1
  STEP=$("$PYTHON" - "$SRC_FPS" "$requested_fps" <<'PY_STEP'
import sys
source = float(sys.argv[1])
target = float(sys.argv[2])
print(max(1, int(round(source / target))))
PY_STEP
)
  ACTUAL_FPS=$("$PYTHON" - "$SRC_FPS" "$STEP" <<'PY_FPS'
import sys
print(f"{float(sys.argv[1]) / int(sys.argv[2]):.6g}")
PY_FPS
)
}

render_gif() {
  local requested_fps=$1
  calculate_timing "$requested_fps"
  printf 'Renderizando %s a aproximadamente %s fps...\n' "$INPUT" "$ACTUAL_FPS" >&2
  if ! "$LOTTIE" --gif-skip-frames "$STEP" "$INPUT" "$GIF" >"$TMP/lottie.log" 2>&1; then
    cat "$TMP/lottie.log" >&2
    exit 1
  fi
}

encode() {
  local quality=$1
  local requested_fps=$2
  ffmpeg -hide_banner -loglevel error -y \
    -ignore_loop 1 -t "$DURATION" -i "$GIF" \
    -vf "fps=${requested_fps}" \
    -c:v libwebp_anim -loop 0 -lossless 0 \
    -quality "$quality" -compression_level 4 -pix_fmt yuva420p \
    "$OUTPUT"
}

# Renderiza Lottie apenas uma vez. Se o WebP ainda exceder 500 KB, o ffmpeg
# reduz os quadros durante a codificacao, sem repetir a etapa vetorial lenta.
render_gif "$TARGET_FPS"

fps_candidates=("$TARGET_FPS")
for fallback in 15 12 10; do
  if (( fallback < TARGET_FPS )); then
    present=0
    for existing in "${fps_candidates[@]}"; do
      [[ "$existing" == "$fallback" ]] && present=1
    done
    (( present == 1 )) || fps_candidates+=("$fallback")
  fi
done

success=0
final_q=$QUALITY
final_size=0
final_fps=""
for requested_fps in "${fps_candidates[@]}"; do
  q=$QUALITY
  while :; do
    encode "$q" "$requested_fps"
    size=$(stat -c %s "$OUTPUT")
    if (( size <= 500000 )); then
      success=1
      final_q=$q
      final_size=$size
      final_fps=$requested_fps
      break 2
    fi
    if (( q <= MIN_QUALITY )); then
      break
    fi
    next_q=$((q - 7))
    (( next_q < MIN_QUALITY )) && next_q=$MIN_QUALITY
    q=$next_q
    echo "WebP com $size bytes; repetindo com qualidade $q..." >&2
  done
  echo "Ainda acima de 500000 bytes a $requested_fps fps; tentando menos quadros..." >&2
done

if (( success == 0 )); then
  echo "Nao foi possivel ficar abaixo de 500000 bytes." >&2
  echo "Ultimo arquivo: $OUTPUT ($size bytes, qualidade $q, $requested_fps fps)." >&2
  echo "Tente TGS2WEBP_MIN_QUALITY=25 ou TGS2WEBP_FPS=10." >&2
  exit 3
fi

printf 'Criado: %s (%s fps, qualidade %d, %d bytes)\n' \
  "$OUTPUT" "$final_fps" "$final_q" "$final_size"
