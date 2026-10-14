use log::error;
use serde::{Deserialize, Serialize};
use serenity::{builder::CreateAttachment, client::Context};

use std::io::BufWriter;
use std::num::NonZeroU32;

use fast_image_resize as fr;

use image::ImageEncoder;

// png인지 확인하는 부울값과 img url을 반환함
pub trait EmojiFilter {
    fn emoji_format_filter(&self) -> Result<(bool, String), ()>;
    fn multi_emoji_format_filter(&self) -> Result<(Layout, Vec<String>), ()>;
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Layout {
    Horizontal, // urls.len() ∈ 2..=4. 모든 이미지 높이 128로 정규화 후 가로로 붙임.
    Vertical,   // urls.len() ∈ 2..=3. 모든 이미지 너비 128로 정규화 후 세로로 붙임.
    Grid2x2,    // urls.len() == 4. 128x128 정규화 후 2x2 배치.
    Grid3x3,    // urls.len() == 9. 128x128 정규화 후 3x3 배치.
}

const CELL: u32 = 128;

type DynErr = Box<dyn std::error::Error + Send + Sync>;

pub async fn merge_multiple_images(urls: &[String], layout: Layout) -> Result<Vec<u8>, DynErr> {
    let expected_ok = match layout {
        Layout::Horizontal => (2..=4).contains(&urls.len()),
        Layout::Vertical => (2..=3).contains(&urls.len()),
        Layout::Grid2x2 => urls.len() == 4,
        Layout::Grid3x3 => urls.len() == 9,
    };
    if !expected_ok {
        return Err(format!(
            "merge_multiple_images: invalid url count {} for layout {:?}",
            urls.len(),
            layout
        )
        .into());
    }

    let images = fetch_all_png(urls).await?;
    let normalized = normalize_images(images, layout);
    encode_canvas(&normalized, layout)
}

async fn fetch_all_png(urls: &[String]) -> Result<Vec<image::DynamicImage>, DynErr> {
    use futures::future::try_join_all;

    let bytes_list = try_join_all(urls.iter().map(|u| async move {
        let resp = reqwest::get(u)
            .await
            .map_err(|e| Box::new(e) as DynErr)?;
        let b = resp.bytes().await.map_err(|e| Box::new(e) as DynErr)?;
        Ok::<_, DynErr>(b)
    }))
    .await?;

    bytes_list
        .into_iter()
        .map(|b| image::load_from_memory(&b).map_err(|e| Box::new(e) as DynErr))
        .collect()
}

fn normalize_images(images: Vec<image::DynamicImage>, layout: Layout) -> Vec<image::RgbaImage> {
    use image::imageops::{resize, FilterType};
    use image::GenericImageView;

    images
        .into_iter()
        .map(|img| {
            let (w, h) = img.dimensions();
            let (new_w, new_h) = match layout {
                Layout::Horizontal => {
                    // height=128, width은 비율 유지
                    let scale = CELL as f32 / h as f32;
                    ((w as f32 * scale).round() as u32, CELL)
                }
                Layout::Vertical => {
                    // width=128, height은 비율 유지
                    let scale = CELL as f32 / w as f32;
                    (CELL, (h as f32 * scale).round() as u32)
                }
                Layout::Grid2x2 | Layout::Grid3x3 => (CELL, CELL),
            };
            let new_w = new_w.max(1);
            let new_h = new_h.max(1);
            resize(&img.to_rgba8(), new_w, new_h, FilterType::Lanczos3)
        })
        .collect()
}

fn encode_canvas(imgs: &[image::RgbaImage], layout: Layout) -> Result<Vec<u8>, DynErr> {
    use image::{imageops, ImageFormat};
    use std::io::Cursor;

    let (canvas_w, canvas_h, placements) = match layout {
        Layout::Horizontal => {
            let total_w: u32 = imgs.iter().map(|i| i.width()).sum();
            let max_h: u32 = imgs.iter().map(|i| i.height()).max().unwrap_or(CELL);
            let mut offset = 0u32;
            let placements: Vec<(u32, u32)> = imgs
                .iter()
                .map(|img| {
                    let x = offset;
                    offset += img.width();
                    (x, 0)
                })
                .collect();
            (total_w, max_h, placements)
        }
        Layout::Vertical => {
            let max_w: u32 = imgs.iter().map(|i| i.width()).max().unwrap_or(CELL);
            let total_h: u32 = imgs.iter().map(|i| i.height()).sum();
            let mut offset = 0u32;
            let placements: Vec<(u32, u32)> = imgs
                .iter()
                .map(|img| {
                    let y = offset;
                    offset += img.height();
                    (0, y)
                })
                .collect();
            (max_w, total_h, placements)
        }
        Layout::Grid2x2 => {
            let placements: Vec<(u32, u32)> = (0..4)
                .map(|i| ((i % 2) as u32 * CELL, (i / 2) as u32 * CELL))
                .collect();
            (CELL * 2, CELL * 2, placements)
        }
        Layout::Grid3x3 => {
            let placements: Vec<(u32, u32)> = (0..9)
                .map(|i| ((i % 3) as u32 * CELL, (i / 3) as u32 * CELL))
                .collect();
            (CELL * 3, CELL * 3, placements)
        }
    };

    let mut canvas = image::RgbaImage::new(canvas_w, canvas_h);
    for (img, (x, y)) in imgs.iter().zip(placements.iter()) {
        imageops::overlay(&mut canvas, img, *x as i64, *y as i64);
    }

    let mut result_buffer = Vec::new();
    {
        let mut cursor = Cursor::new(&mut result_buffer);
        canvas
            .write_to(&mut cursor, ImageFormat::Png)
            .map_err(|e| Box::new(e) as DynErr)?;
    }
    Ok(result_buffer)
}
