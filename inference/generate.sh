python -m grpc_tools.protoc -I. --python_out=src --pyi_out=src --grpc_python_out=src ./inference.proto
