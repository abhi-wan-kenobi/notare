MODEL_REPO="https://huggingface.co/lmstudio-community/Llama-3.2-3B-Instruct-GGUF"
LOCAL_DIR="./llama-3.2-3b-instruct-gguf"

GIT_LFS_SKIP_SMUDGE=1 git clone --depth 1 $MODEL_REPO $LOCAL_DIR
cd $LOCAL_DIR
git lfs pull
