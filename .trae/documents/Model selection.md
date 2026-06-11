
## paraformer-large

Paraformer-large长音频模型集成VAD、ASR、标点与时间戳功能，可直接对时长为数小时音频进行识别，并输出带标点文字与时间戳：
- ASR模型：Parformer-large模型结构为非自回归语音识别模型，多个中文公开数据集上取得SOTA效果，可快速地基于ModelScope对模型进行微调定制和推理。
- 热词版本：Paraformer-large热词版模型支持热词定制功能，基于提供的热词列表进行激励增强，提升热词的召回率和准确率。
- https://k2-fsa.github.io/sherpa/onnx/paraformer/index.html#streaming-paraformer
- https://www.modelscope.cn/models/iic/speech_paraformer-large_asr_nat-zh-cn-16k-common-vocab8404-onnx/files
- https://k2-fsa.github.io/sherpa/onnx/pretrained_models/offline-paraformer/paraformer-models.html#csukuangfj-sherpa-onnx-paraformer-trilingual-zh-cantonese-en-chinese-english-cantonese
- https://github.com/k2-fsa/sherpa-onnx/releases/download/asr-models/sherpa-onnx-paraformer-zh-2024-03-09.tar.bz2
- https://www.modelscope.cn/models/QuadraV/speech_paraformer-large_asr_nat-zh-cantonese-en-16k-vocab8501-online-onnx/files  推荐下载地址
- 我把模型上传到了 https://gitcode.com/tabortao/VelociText/releases/download/v0.1.2/paraformer.zip  推荐下载地址，用户可以直接下载并解压到 `paraformer/` 目录下


## SenseVoice-Small

**SenseVoice**专注于高精度多语言语音识别、情感辨识和音频事件检测

- **多语言识别：** 采用超过40万小时数据训练，支持超过50种语言，识别效果上优于Whisper模型。
- **富文本识别：**
    - 具备优秀的情感识别，能够在测试数据上达到和超过目前最佳情感识别模型的效果。
    - 支持声音事件检测能力，支持音乐、掌声、笑声、哭声、咳嗽、喷嚏等多种常见人机交互事件进行检测。
- **高效推理：** SenseVoice-Small模型采用非自回归端到端框架，推理延迟极低，10s音频推理仅耗时70ms，15倍优于Whisper-Large。
- **微调定制：** 具备便捷的微调脚本与策略，方便用户根据业务场景修复长尾样本问题。
- **服务部署：** 具有完整的服务部署链路，支持多并发请求，支持客户端语言有，python、c++、html、java与c#等。
- https://k2-fsa.github.io/sherpa/onnx/sense-voice/index.html
- https://www.modelscope.cn/models/iic/SenseVoiceSmall-onnx

