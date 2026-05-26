use log::error;
use serde::{Deserialize, Serialize};
use serenity::{builder::CreateAttachment, client::Context};

use std::io::BufWriter;
use std::num::NonZeroU32;

use fast_image_resize as fr;

use image::ImageEncoder;

use lazy_static::lazy_static;
use regex::Regex;

// png인지 확인하는 부울값과 img url을 반환함
pub trait EmojiFilter {
    fn emoji_format_filter(&self) -> Result<(bool, String), ()>;
    fn double_emoji_format_filter(&self) -> Result<(bool, String, String), ()>;
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum ImageSize {
    HyperTechniqueOfLisaSuFinger,           //16x16
    Small,                                  //64x64
    Medium,                                 //기본 사이즈(128x128)
    Large,                                  //256x256
    HyperSuperUltraSexFeaturedFuckingLarge, //300x300
    Auto,
}

impl ImageSize {
    pub fn string_to_value(input_str: &str) -> Self {
        match input_str {
            "HyperTechniqueOfLisaSuFinger" => Self::HyperTechniqueOfLisaSuFinger,
            "Small" => Self::Small,
            "Medium" => Self::Medium,
            "Large" => Self::Large,
            "HyperSuperUltraSexFeaturedFuckingLarge" => {
                Self::HyperSuperUltraSexFeaturedFuckingLarge
            }
            _ => Self::Auto,
        }
    }

    pub fn value_to_string(input_value: &ImageSize) -> String {
        let st = match input_value {
            Self::HyperTechniqueOfLisaSuFinger => "HyperTechniqueOfLisaSuFinger",
            Self::Small => "Small",
            Self::Medium => "Medium",
            Self::Large => "Large",
            Self::HyperSuperUltraSexFeaturedFuckingLarge => {
                "HyperSuperUltraSexFeaturedFuckingLarge"
            }
            Self::Auto => "Auto",
        };
        st.to_string()
    }
}

// img_url은 항상 PNG/WebP 파일임. 실패 시 None — 호출자가 fallback 처리.
pub async fn get_resized_image(
    ctx: &Context,
    img_url: &str,
    img_size: &ImageSize,
) -> Option<CreateAttachment> {
    match img_size {
        ImageSize::Auto => match CreateAttachment::url(ctx.http.clone(), img_url).await {
            Ok(att) => Some(att),
            Err(why) => {
                error!("CreateAttachment::url failed for {}: {:?}", img_url, why);
                None
            }
        },
        ImageSize::HyperTechniqueOfLisaSuFinger => resize_png(img_url, 16, 16).await,
        ImageSize::Small => resize_png(img_url, 64, 64).await,
        ImageSize::Medium => resize_png(img_url, 128, 128).await,
        ImageSize::Large => resize_png(img_url, 256, 256).await,
        ImageSize::HyperSuperUltraSexFeaturedFuckingLarge => resize_png(img_url, 300, 300).await,
    }
}

lazy_static! {
    static ref SINGLE_EMOJI_RE: Regex = Regex::new(r"^<a?:.+?:\d+>$").unwrap();
}

pub fn emoji_format_filter(emoji_string: &str) -> Result<(bool, String), ()> {
    let msg_content_vec: Vec<&str> = emoji_string.split(':').collect();
    if !SINGLE_EMOJI_RE.is_match(emoji_string) || msg_content_vec.len() != 3 {
        return Err(());
    }
    let mut id = msg_content_vec[2].to_string();
    id.pop();
    let mut is_png = false;
    let img_url = format!(
        "https://cdn.discordapp.com/emojis/{}.{}",
        id,
        if emoji_string.contains("<a:") {
            "gif"
        } else {
            is_png = true;
            "png"
        }
    );
    Ok((is_png, img_url))
}

use image::{codecs::png::PngEncoder, ColorType};

async fn resize_png(img_url: &str, dst_w: u32, dst_h: u32) -> Option<CreateAttachment> {
    let dst_width = NonZeroU32::new(dst_w)?;
    let dst_height = NonZeroU32::new(dst_h)?;

    let bytes = match reqwest::get(img_url).await {
        Ok(resp) => match resp.bytes().await {
            Ok(b) => b,
            Err(why) => {
                error!("read body failed ({}): {:?}", img_url, why);
                return None;
            }
        },
        Err(why) => {
            error!("fetch failed ({}): {:?}", img_url, why);
            return None;
        }
    };

    let img = match image::load_from_memory(&bytes) {
        Ok(i) => i,
        Err(why) => {
            error!("image decode failed ({}): {:?}", img_url, why);
            return None;
        }
    };

    let width = NonZeroU32::new(img.width())?;
    let height = NonZeroU32::new(img.height())?;

    let mut src_image =
        match fr::Image::from_vec_u8(width, height, img.to_rgba8().into_raw(), fr::PixelType::U8x4)
        {
            Ok(i) => i,
            Err(why) => {
                error!("fr::Image build failed: {:?}", why);
                return None;
            }
        };

    let alpha_mul_div = fr::MulDiv::default();
    if let Err(why) = alpha_mul_div.multiply_alpha_inplace(&mut src_image.view_mut()) {
        error!("alpha pre-multiply failed: {:?}", why);
        return None;
    }

    let mut dst_image = fr::Image::new(dst_width, dst_height, src_image.pixel_type());

    let mut dst_view = dst_image.view_mut();

    let mut resizer = fr::Resizer::new(fr::ResizeAlg::Convolution(fr::FilterType::Bilinear));
    if let Err(why) = resizer.resize(&src_image.view(), &mut dst_view) {
        error!("resize failed: {:?}", why);
        return None;
    }

    if let Err(why) = alpha_mul_div.divide_alpha_inplace(&mut dst_view) {
        error!("alpha post-divide failed: {:?}", why);
        return None;
    }

    let mut result_buf = BufWriter::new(Vec::new());
    if let Err(why) = PngEncoder::new(&mut result_buf).write_image(
        dst_image.buffer(),
        dst_width.get(),
        dst_height.get(),
        ColorType::Rgba8,
    ) {
        error!("png encode failed: {:?}", why);
        return None;
    }

    let inner = match result_buf.into_inner() {
        Ok(v) => v,
        Err(why) => {
            error!("buf flush failed: {:?}", why);
            return None;
        }
    };

    Some(CreateAttachment::bytes(inner, "resized.png".to_string()))
}

pub async fn merge_two_emojis(
    first_url: &str,
    second_url: &str,
) -> Result<Vec<u8>, Box<dyn std::error::Error + Send + Sync>> {
    use image::{imageops, GenericImageView, ImageFormat};
    use std::io::Cursor;

    let first_emoji_url = format!("{}?size=128", first_url);
    let second_emoji_url = format!("{}?size=128", second_url);

    let (first_emoji_result, second_emoji_result) = tokio::join!(
        async {
            reqwest::get(&first_emoji_url)
                .await
                .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?
                .bytes()
                .await
                .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
        },
        async {
            reqwest::get(&second_emoji_url)
                .await
                .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?
                .bytes()
                .await
                .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
        }
    );

    let first_emoji_bytes = first_emoji_result?.to_vec();
    let second_emoji_bytes = second_emoji_result?.to_vec();

    let first_img = image::load_from_memory(&first_emoji_bytes)
        .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;
    let second_img = image::load_from_memory(&second_emoji_bytes)
        .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;

    let (width1, height1) = first_img.dimensions();
    let (width2, height2) = second_img.dimensions();

    let mut merged_img = image::RgbaImage::new(width1 + width2, std::cmp::max(height1, height2));

    imageops::overlay(&mut merged_img, &first_img, 0, 0);
    imageops::overlay(&mut merged_img, &second_img, width1 as i64, 0);

    let mut result_buffer = Vec::new();
    {
        let mut cursor = Cursor::new(&mut result_buffer);
        merged_img
            .write_to(&mut cursor, ImageFormat::Png)
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;
    }

    Ok(result_buffer)
}
