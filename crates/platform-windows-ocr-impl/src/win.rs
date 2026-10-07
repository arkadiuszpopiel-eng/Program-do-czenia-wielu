//! Wywołania WinRT (`Windows.Media.Ocr`, `Windows.Graphics.Imaging`, `Windows.Storage.Streams`).

use tools_vision_contract::{OcrError, OcrLine, OcrRect, OcrRequest, OcrText, OcrWord};
use windows::Globalization::Language;
use windows::Graphics::Imaging::{
    BitmapAlphaMode, BitmapDecoder, BitmapInterpolationMode, BitmapPixelFormat, BitmapTransform,
    ColorManagementMode, ExifOrientationMode,
};
use windows::Media::Ocr::{OcrEngine, OcrResult};
use windows::Storage::Streams::{DataWriter, InMemoryRandomAccessStream};
use windows::core::HSTRING;

use crate::fit_factor;

fn failed(stage: &'static str) -> impl Fn(windows::core::Error) -> OcrError {
    move |e| OcrError::Failed(format!("{stage}: {}", e.message()))
}

fn image_error(e: windows::core::Error) -> OcrError {
    OcrError::Image(e.message())
}

fn stream_of(bytes: &[u8]) -> Result<InMemoryRandomAccessStream, OcrError> {
    let stream = InMemoryRandomAccessStream::new().map_err(failed("strumień"))?;
    let writer = DataWriter::CreateDataWriter(&stream).map_err(failed("DataWriter"))?;
    writer.WriteBytes(bytes).map_err(failed("zapis bajtów"))?;
    writer
        .StoreAsync()
        .and_then(|op| op.join())
        .map_err(failed("zapis strumienia"))?;
    writer
        .FlushAsync()
        .and_then(|op| op.join())
        .map_err(failed("opróżnienie strumienia"))?;
    writer.DetachStream().map_err(failed("odłączenie"))?;
    stream.Seek(0).map_err(failed("przewinięcie"))?;
    Ok(stream)
}

fn engine(language: Option<&str>) -> Result<OcrEngine, OcrError> {
    match language {
        Some(tag) => {
            let missing = || OcrError::Language(tag.to_owned());
            let lang = Language::CreateLanguage(&HSTRING::from(tag)).map_err(|_| missing())?;
            if !OcrEngine::IsLanguageSupported(&lang).unwrap_or(false) {
                return Err(missing());
            }
            OcrEngine::TryCreateFromLanguage(&lang).map_err(|_| missing())
        }
        None => OcrEngine::TryCreateFromUserProfileLanguages()
            .map_err(|e| OcrError::Language(format!("języki profilu: {}", e.message()))),
    }
}

fn lines_of(result: &OcrResult, back: f32) -> Result<Vec<OcrLine>, OcrError> {
    let read = failed("odczyt wyniku");
    let lines = result.Lines().map_err(&read)?;
    let mut out = Vec::new();
    for i in 0..lines.Size().map_err(&read)? {
        let line = lines.GetAt(i).map_err(&read)?;
        let words = line.Words().map_err(&read)?;
        let mut ws = Vec::new();
        for j in 0..words.Size().map_err(&read)? {
            let word = words.GetAt(j).map_err(&read)?;
            let r = word.BoundingRect().map_err(&read)?;
            ws.push(OcrWord {
                text: word.Text().map_err(&read)?.to_string(),
                rect: OcrRect {
                    x: r.X * back,
                    y: r.Y * back,
                    width: r.Width * back,
                    height: r.Height * back,
                },
            });
        }
        out.push(OcrLine {
            text: line.Text().map_err(&read)?.to_string(),
            words: ws,
        });
    }
    Ok(out)
}

/// Rozpoznaje tekst obrazu (wymiary sprawdzane przed dekodowaniem pikseli).
pub(crate) fn recognize(request: &OcrRequest, max_pixels: u64) -> Result<OcrText, OcrError> {
    let stream = stream_of(&request.image)?;
    let decoder = BitmapDecoder::CreateAsync(&stream)
        .and_then(|op| op.join())
        .map_err(image_error)?;
    let width = decoder.PixelWidth().map_err(image_error)?;
    let height = decoder.PixelHeight().map_err(image_error)?;
    if width == 0 || height == 0 || u64::from(width) * u64::from(height) > max_pixels {
        return Err(OcrError::TooLarge { width, height });
    }
    let max_side = OcrEngine::MaxImageDimension().map_err(failed("limit silnika"))?;
    let factor = fit_factor(width, height, max_side);
    let transform = BitmapTransform::new().map_err(failed("transformacja"))?;
    if factor < 1.0 {
        let scaled = |v: u32| ((f64::from(v) * factor).floor() as u32).max(1);
        transform
            .SetScaledWidth(scaled(width))
            .and_then(|()| transform.SetScaledHeight(scaled(height)))
            .and_then(|()| transform.SetInterpolationMode(BitmapInterpolationMode::Fant))
            .map_err(failed("skalowanie"))?;
    }
    let bitmap = decoder
        .GetSoftwareBitmapTransformedAsync(
            BitmapPixelFormat::Bgra8,
            BitmapAlphaMode::Premultiplied,
            &transform,
            ExifOrientationMode::IgnoreExifOrientation,
            ColorManagementMode::DoNotColorManage,
        )
        .and_then(|op| op.join())
        .map_err(image_error)?;
    let engine = engine(request.language.as_deref())?;
    let result = engine
        .RecognizeAsync(&bitmap)
        .and_then(|op| op.join())
        .map_err(failed("rozpoznawanie"))?;
    let language = engine
        .RecognizerLanguage()
        .and_then(|l| l.LanguageTag())
        .map(|t| t.to_string())
        .unwrap_or_default();
    let angle = result.TextAngle().and_then(|a| a.Value()).ok();
    Ok(OcrText {
        language,
        lines: lines_of(&result, (1.0 / factor) as f32)?,
        angle,
    })
}

/// Języki z zainstalowanymi pakietami OCR.
pub(crate) fn languages() -> Result<Vec<String>, OcrError> {
    let read = failed("języki OCR");
    let list = OcrEngine::AvailableRecognizerLanguages().map_err(&read)?;
    let mut out = Vec::new();
    for i in 0..list.Size().map_err(&read)? {
        out.push(
            list.GetAt(i)
                .and_then(|l| l.LanguageTag())
                .map_err(&read)?
                .to_string(),
        );
    }
    Ok(out)
}
