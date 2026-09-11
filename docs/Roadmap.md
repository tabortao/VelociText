## Planning

- [X] 增加paraformer large onnx模型支持，https://k2-fsa.github.io/sherpa/onnx/paraformer/index.html
- [ ] 增加词典(hotwords)功能:https://k2-fsa.github.io/sherpa/onnx/hotwords/index.html ，已知问题，专有名词（热词）还不起作用，暂时推荐使用替换词典。
- [ ] 智能标点：https://k2-fsa.github.io/sherpa/onnx/punctuation/index.html
- [ ] 增加AI优化转录结果的功能
- [X] 参考 https://github.com/k2-fsa/sherpa-onnx/blob/master/tauri-examples/non-streaming-speech-recognition-from-file/README.md 进行改进。
- [X] symphonia 替代 FFmpeg：纯 Rust 实现，无系统依赖，可直接打包进 Tauri 应用，支持从视频容器中提取音频 
- [X] 项目是否不依赖 FFmpeg，后续考虑去除 FFmpeg 依赖
- [X] 增加qwen3-asr模型支持
- [X] [sherpa-onnx模型下载地址1](https://gitcode.com/tabortao/VelociText/releases/model)
- [X] [sherpa-onnx模型下载地址2](https://www.modelscope.cn/models/tabortao/sherpa-onnx-asr-int8/summary)，自己从sherpa-onnx收集整理的模型，上传到modelscope方便在项目中下载使用。AI建议推荐保持现有方案，暂不切换下载链接。 Qwen3-ASR 的 tokenizer/ 目录有 30+ 个文件，ModelScope 逐个下载会非常脆弱且慢。单 zip 下载解压更稳定。如果 gitcode.com 未来出现可用性问题，再切换到 ModelScope 方案。我目前在modelscope也上传了zip压缩文件，有问题了后面替换。进入到要下载的zip文件后，可以复制到下载地址，例如https://www.modelscope.cn/models/tabortao/sherpa-onnx-asr-int8/resolve/master/sherpa-onnx-paraformer-trilingual-zh-cantonese-en-int8.zip
- [X] 项目VAD功能，各个模型是否都用了 [silero-vad](https://k2-fsa.github.io/sherpa/onnx/vad/silero-vad.html) 模型。
- [X] 项目添加OCR功能：https://aistudio.baidu.com/paddleocr
- [X] 项目OCR截图可参考 https://github.com/mg-chao/snow-shot
- [X] 项目添加系统托盘功能，支持最小化到托盘，从托盘打开应用，退出应用。
- [X] 添加批量OCR功能(已取消该功能，专注做ASR功能)。
- [ ] 提供API接口，支持其他应用调用ASR等功能。https://tauri.app/plugin/http-client/
- [ ] 提供CLI工具，支持批量行调用ASR等功能。https://tauri.app/plugin/cli/
- [X] 提供PDF文档OCR功能。依赖https://github.com/ajrcarey/pdfium-render 和 https://github.com/bblanchon/pdfium-binaries
- [X] 支持qwen3-asr-1.7B模型
- [X] 支持转音频、转字幕功能

