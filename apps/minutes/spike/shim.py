import sys,types,dataclasses
@dataclasses.dataclass
class AudioMetaData:
    sample_rate:int=0;num_frames:int=0;num_channels:int=0;bits_per_sample:int=0;encoding:str=""
b=types.ModuleType("torchaudio.backend");c=types.ModuleType("torchaudio.backend.common");c.AudioMetaData=AudioMetaData
b.common=c;sys.modules["torchaudio.backend"]=b;sys.modules["torchaudio.backend.common"]=c
