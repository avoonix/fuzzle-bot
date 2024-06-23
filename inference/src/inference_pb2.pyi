from google.protobuf.internal import containers as _containers
from google.protobuf.internal import enum_type_wrapper as _enum_type_wrapper
from google.protobuf import descriptor as _descriptor
from google.protobuf import message as _message
from typing import ClassVar as _ClassVar, Iterable as _Iterable, Optional as _Optional, Union as _Union

DESCRIPTOR: _descriptor.FileDescriptor

class TextModel(int, metaclass=_enum_type_wrapper.EnumTypeWrapper):
    __slots__ = ()
    CLIP_TEXT: _ClassVar[TextModel]

class ImageModel(int, metaclass=_enum_type_wrapper.EnumTypeWrapper):
    __slots__ = ()
    CLIP_IMAGE: _ClassVar[ImageModel]
CLIP_TEXT: TextModel
CLIP_IMAGE: ImageModel

class TextEmbeddingRequest(_message.Message):
    __slots__ = ("model", "text")
    MODEL_FIELD_NUMBER: _ClassVar[int]
    TEXT_FIELD_NUMBER: _ClassVar[int]
    model: TextModel
    text: str
    def __init__(self, model: _Optional[_Union[TextModel, str]] = ..., text: _Optional[str] = ...) -> None: ...

class ImageEmbeddingRequest(_message.Message):
    __slots__ = ("model", "image")
    MODEL_FIELD_NUMBER: _ClassVar[int]
    IMAGE_FIELD_NUMBER: _ClassVar[int]
    model: ImageModel
    image: bytes
    def __init__(self, model: _Optional[_Union[ImageModel, str]] = ..., image: _Optional[bytes] = ...) -> None: ...

class EmbeddingResponse(_message.Message):
    __slots__ = ("embedding",)
    EMBEDDING_FIELD_NUMBER: _ClassVar[int]
    embedding: _containers.RepeatedScalarFieldContainer[float]
    def __init__(self, embedding: _Optional[_Iterable[float]] = ...) -> None: ...
