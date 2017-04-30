use hyper::net::HttpsConnector;
use hyper::Client;
use hyper_native_tls::NativeTlsClient;

use gdk_pixbuf::{Pixbuf, PixbufLoader, InterpType};


use hyper::Url;


use std::io::{BufReader, Read};

/// Make a HTTPS-compatible client
pub fn make_client() -> Client {
    let ssl = NativeTlsClient::new().unwrap();
    let connector = HttpsConnector::new(ssl);
    Client::with_connector(connector)
}

/// Load images from urls, resizing to a certain width
pub fn load_pixbufs(urls: &[Url], max_width: i32) -> Vec<Option<Pixbuf>> {
    let client = make_client();
    let mut bytes = Vec::with_capacity(512);
    urls.iter()
        .map(|url| {
            let loader = PixbufLoader::new();
            if let Ok(input) = client.get(url.clone()).send() {
                let mut reader = BufReader::new(input);
                bytes.clear();
                reader.read_to_end(&mut bytes).unwrap();
                loader.loader_write(&bytes).unwrap();
                loader.close().unwrap();
                let mut image = loader.get_pixbuf().unwrap();
                let (width, height) = (image.get_width(), image.get_height());
                if width > max_width {
                    let (new_width, new_height) = (max_width, (height * max_width) / width);
                    image = image
                        .scale_simple(new_width, new_height, InterpType::Bilinear)
                        .unwrap();
                }
                Some(image)
            } else {
                None
            }
        })
        .collect()
}
