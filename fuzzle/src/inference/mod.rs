use crate::{bot::{BotError, InternalError}, inference::inference::{generate_client::GenerateClient, ImageEmbeddingRequest, TextEmbeddingRequest, TextModel}};

mod inference;

#[tracing::instrument(err(Debug))]
pub async fn text_to_clip_embedding(text: String, grpc_url: String) -> Result<Vec<f32>, InternalError> {
    // TODO: connection pool
    let mut client = GenerateClient::connect(grpc_url).await?;

    let request = tonic::Request::new(TextEmbeddingRequest {
        model: TextModel::ClipText.into(),
        text,
    });

    let response = client.text_embedding(request).await?;

    Ok(response.into_inner().embedding)
}

#[tracing::instrument(skip(image), err(Debug))]
pub async fn image_to_clip_embedding(image: Vec<u8>, grpc_url: String) -> Result<Vec<f32>, InternalError> {
    let mut client = GenerateClient::connect(grpc_url).await?;

    let request = tonic::Request::new(ImageEmbeddingRequest {
        model: TextModel::ClipText.into(),
        image,
    });

    let response = client.image_embedding(request).await?;

    Ok(response.into_inner().embedding)
}