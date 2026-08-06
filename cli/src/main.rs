use pixelizer_core::PixelizerError;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let pipeline_path = &args[1];
    let input_path = &args[2];
    let output_path = &args[3];

    let raw_yaml = std::fs::read_to_string(pipeline_path).expect("read pipeline");
    let raw_pic = std::fs::read(input_path).expect("read image");
    let pipeline = make_pipeline(raw_yaml);
    let pic = make_pic(raw_pic);
    let result = pixelizer_core::apply(&pipeline, pic);
    match result {
        Ok(output) => {
            // Use core's encode_png (indexed PNG when the image is small and
            // opaque) rather than image::save, which always writes truecolor.
            let bytes = pixelizer_core::encode_png(&output);
            std::fs::write(output_path, bytes).expect("write output");
        }
        // Apparently, using thiserror could clean this up.
        Err(error) => match error {
            PixelizerError::HexParseError(e) => eprintln!("{}", e),
            PixelizerError::NoColorsError(e) => eprintln!("{}", e),
            PixelizerError::PosterizeError(e) => eprintln!("{}", e),
        },
    }
}

fn make_pipeline(yaml: String) -> pixelizer_core::Pipeline {
    serde_yaml::from_str(&yaml).expect("parse pipeline")
}

fn make_pic(bytes: Vec<u8>) -> pixelizer_core::Image {
    pixelizer_core::image::load_from_memory(&bytes)
        .expect("decode")
        .to_rgba8()
}
