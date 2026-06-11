## Planning

- [X] 增加paraformer large onnx模型支持，https://k2-fsa.github.io/sherpa/onnx/paraformer/index.html
- [ ] 增加词典功能
- [ ] 增加AI优化转录结果的功能
- [X] 参考 https://github.com/k2-fsa/sherpa-onnx/blob/master/tauri-examples/non-streaming-speech-recognition-from-file/README.md 进行改进。
- [X] symphonia 替代 FFmpeg：纯 Rust 实现，无系统依赖，可直接打包进 Tauri 应用，支持从视频容器中提取音频 
- [ ] 项目是否不依赖 FFmpeg，后续考虑去除 FFmpeg 依赖
- [X] 增加qwen3-asr模型支持
- [X] [sherpa-onnx模型下载地址1](https://gitcode.com/tabortao/VelociText/releases/model)
- [ ] [sherpa-onnx模型下载地址2](https://www.modelscope.cn/models/tabortao/sherpa-onnx-asr-int8/summary)，自己从sherpa-onnx收集整理的模型，上传到modelscope方便在项目中下载使用。AI建议推荐保持现有方案，暂不切换下载链接。 Qwen3-ASR 的 tokenizer/ 目录有 30+ 个文件，ModelScope 逐个下载会非常脆弱且慢。单 zip 下载解压更稳定。如果 gitcode.com 未来出现可用性问题，再切换到 ModelScope 方案。我目前在modelscope也上传了zip压缩文件，有问题了后面替换。进入到要下载的zip文件后，可以复制到下载地址，例如https://www.modelscope.cn/models/tabortao/sherpa-onnx-asr-int8/resolve/master/sherpa-onnx-paraformer-trilingual-zh-cantonese-en-int8.zip

