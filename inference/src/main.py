from concurrent import futures
import grpc
import inference_pb2
import inference_pb2_grpc
from transformers import AutoProcessor, AutoModelForZeroShotImageClassification, AutoTokenizer
from PIL import Image
import torch
from io import BytesIO
import os

model_name = os.environ.get("FUZZLE_INFERENCE_MODEL_NAME")
port = os.environ.get("FUZZLE_INFERENCE_PORT")
if model_name is None:
    print("FUZZLE_INFERENCE_MODEL_NAME missing")
    exit(1)
if port is None:
    print("FUZZLE_INFERENCE_PORT missing")
    exit(1)

processor = AutoProcessor.from_pretrained(model_name)
model = AutoModelForZeroShotImageClassification.from_pretrained(model_name)
tokenizer = AutoTokenizer.from_pretrained(model_name)


# TODO: batch requests?
class Generator(inference_pb2_grpc.GenerateServicer):
    def TextEmbedding(self, request: inference_pb2.TextEmbeddingRequest, context):
        assert request.model == inference_pb2.TextModel.CLIP_TEXT
        inputs = tokenizer([request.text], padding=True, return_tensors="pt")
        with torch.no_grad():
            text_features = model.get_text_features(**inputs)
        return inference_pb2.EmbeddingResponse(embedding=text_features[0])

    def ImageEmbedding(self, request: inference_pb2.ImageEmbeddingRequest, context):
        assert request.model == inference_pb2.ImageModel.CLIP_IMAGE
        image = Image.open(BytesIO(request.image))
        inputs = processor(images=[image], return_tensors="pt")
        with torch.no_grad():
            image_features = model.get_image_features(**inputs)
        return inference_pb2.EmbeddingResponse(embedding=image_features[0])


def serve():
    server = grpc.server(futures.ThreadPoolExecutor(max_workers=4))
    inference_pb2_grpc.add_GenerateServicer_to_server(Generator(), server)
    server.add_insecure_port("0.0.0.0:" + port)
    server.start()
    print("Server started, listening on " + port)
    server.wait_for_termination()


if __name__ == "__main__":
    serve()
