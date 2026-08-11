use pixelizer_core::MappingSpace;
use pixelizer_core::PixelizerError;

fn main() {
    let args: Vec<String> = std::env::args().collect();

    // Pull out the optional --rgb flag; keep the positional args intact.
    let use_rgb = args.iter().any(|a| a == "--rgb");
    let positional: Vec<&String> = args
        .iter()
        .skip(1)
        .filter(|a| !a.starts_with("--"))
        .collect();

    let pipeline_path = positional[0];
    let input_path = positional[1];
    let output_path = positional[2];

    let space = if use_rgb {
        MappingSpace::Rgb
    } else {
        MappingSpace::Oklab
    };

    let raw_yaml = std::fs::read_to_string(pipeline_path).expect("read pipeline");
    let raw_pic = std::fs::read(input_path).expect("read image");
    let pipeline = make_pipeline(raw_yaml);
    let pic = make_pic(raw_pic);
    let result = pixelizer_core::apply_with_space(&pipeline, pic, space);
    match result {
        Ok(output) => {
            let bytes = pixelizer_core::encode_png(&output);
            std::fs::write(output_path, bytes).expect("write output");
        }
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
