## Planning

- [ ] 增加paraformer large onnx模型支持，https://k2-fsa.github.io/sherpa/onnx/paraformer/index.html
- [ ] 增加词典功能
- [ ] 增加AI优化转录结果的功能
- [ ] 参考 https://github.com/k2-fsa/sherpa-onnx/blob/master/tauri-examples/non-streaming-speech-recognition-from-file/README.md 进行改进。
- [X] symphonia 替代 FFmpeg：纯 Rust 实现，无系统依赖，可直接打包进 Tauri 应用，支持从视频容器中提取音频 
- [ ] 项目是否不依赖 FFmpeg，后续考虑去除 FFmpeg 依赖
