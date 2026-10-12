# 販売候補のモデルと権利確認（2026-10-12）

既存の legal/ と resources/THIRD_PARTY_NOTICES.txt に全文を保持。アプリの設定から表示可能。

|構成|許諾|一次情報・条件|
|---|---|---|
|Whisper small / large-v3-turbo、GGML量子化|MIT|https://github.com/openai/whisper/blob/main/LICENSE とREADMEでコード・重みともMIT。著作権・許諾文を同梱|
|Qwen3-4B-Instruct-2507、Unsloth GGUF Q4_K_M|Apache-2.0|https://huggingface.co/Qwen/Qwen3-4B-Instruct-2507/blob/main/LICENSE ; https://huggingface.co/unsloth/Qwen3-4B-Instruct-2507-GGUF/raw/main/README.md 。許諾文・量子化元の表示|
|WeSpeaker ResNet34-LM ONNX|CC BY 4.0|https://huggingface.co/Wespeaker/wespeaker-voxceleb-resnet34-LM/raw/main/README.md 。モデルカードの明示許諾を確認。帰属・リンク・改変なし表示が必要|
|nnnoiseless内蔵RNNoise|BSD-3-Clause|バージョン0.5.2のソース内許諾をローカルで確認。著作権・許諾・免責の同梱|
|whisper.cpp / llama.cpp / ONNX Runtime|MIT|各同梱配布物の許諾文・第三者表記を保持|
|Symphonia 0.5.5|MPL-2.0|改変していない対象ソースの入手先 https://github.com/pdeljanov/Symphonia を表記し、MPL全文を同梱|

ffmpegを製品には同梱しない。音声復号はSymphonia。ReazonSpeech・kotoba-whisper・DeepFilterNet・pyannoteは同梱しない。テスト合成音声もアプリに同梱しない。

WeSpeakerはモデルカードのCC BY 4.0という明示許諾を一次情報で確認。CC BY 4.0は帰属表示等を条件に商用利用・再配布を認める。旧資料の「モデルの商用許諾は要確認」という状態は、この確認で更新する。学習元VoxCeleb2の公式ページはメタデータにCC BY-SA 4.0を表示しているが、アプリはデータセットを同梱しない。モデルと学習データの許諾を混同しない。

確認元: https://creativecommons.org/licenses/by/4.0/ ; https://www.robots.ox.ac.uk/~vgg/data/voxceleb/vox2.html 。モデルの帰属・ライセンス本文・改変なし表示はlegal/wespeaker-model-NOTICE.txtにあり、同梱表記へ取り込まれる。

本書は全推移依存監査の完了を意味しない。ロックファイルから全文表記を再生成し、ビルド中に増えた依存・Swift StoreKitのOS利用条件も確認する。
