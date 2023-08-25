use std::{self, collections::HashMap, fs, io::Write};

use futures::future::try_join_all;
use std::process;
use reqwest;
use tokio;

struct Manejador {
    cliente: reqwest::Client,
    extensiones: Vec<String>,
    extensiones_verificadas: HashMap<String,String>,
    descargas: HashMap<String,String>
}

async fn descargar(cliente: &reqwest::Client, extension:String, url:String) -> Result<String,String> {
    let peticion = cliente.get(url);
    let respuesta;
    match peticion.send().await {
        Err(error) => {
            let mensaje_error = format!("ERROR: no se ha podido realizar la petición para la descarga de la extension {extension}: {error}");
            return Err(mensaje_error);
        }
        Ok(ok) => {
            respuesta = ok;
        }
    }
    if respuesta.status().as_u16() != 200 {
        let mensaje_error = format!("ERROR: la petición para obtener el archivo de la extension {extension} ha devuelto un status code incorrecto {}", respuesta.status());
        return Err(mensaje_error);
    };

    let cabeceras = respuesta.headers();
    let info_archivo = cabeceras.get(reqwest::header::CONTENT_DISPOSITION);
    let mut nombre_archivo = format!("nuevo_archivo.{extension}");
    match info_archivo {
        Some(ok) => {
            let valor = ok.to_str().unwrap_or("");
            if valor != "" {
                let nombre_esperado = valor.replace("attachment; filename=", "").replace(";", "");
                nombre_archivo = nombre_esperado;
            }
        }
        None => ()
    }

    let contenido;
    match respuesta.bytes().await {
        Err(error) => {
            let mensaje_error = format!("ERROR: no se ha obtener el contenido de la petición para la extension {extension}: {error}");
            return Err(mensaje_error);
        }
        Ok(ok) => {
            contenido = ok;
        }
    }

    let mut fichero;
    match fs::File::create(nombre_archivo.clone()) {
        Err(error) => {
            let mensaje_error = format!("ERROR: no se ha podido crear el archivo {nombre_archivo}: {error}",);
            return Err(mensaje_error);
        }
        Ok(ok) => {
            fichero = ok;
        }
    }
    match fichero.write_all(&contenido) {
        Err(error) => {
            let mensaje_error = format!("ERROR: no se ha podido escribir el archivo {nombre_archivo}: {error}",);
            return Err(mensaje_error);
        }
        Ok(_) => () 
    }

    Ok(nombre_archivo)
}

async fn obtener_url_descarga(cliente: &reqwest::Client, extension: String, url: String) -> Result<HashMap<String,String>,String> {
    let peticion = cliente.get(url);
    let respuesta;
    match peticion.send().await {
        Err(error) => {
            let mensaje_error = format!("ERROR: no se ha podido realizar la petición para la extension {extension}: {error}");
            return Err(mensaje_error);
        }
        Ok(ok) => {
            respuesta = ok;
        }
    }
    if respuesta.status().as_u16() != 200 {
        let mensaje_error = format!("ERROR: la petición para obtener la url de descarga ha devuelto un status code incorrecto {}", respuesta.status());
        return Err(mensaje_error);
    };

    let contenido;
    match respuesta.text().await {
        Err(error) => {
            let mensaje_error = format!("ERROR: no se ha obtener el contenido de la petición para la extension {extension}: {error}");
            return Err(mensaje_error);
        }
        Ok(ok) => {
            contenido = ok;
        }
    }
    let html = scraper::Html::parse_document(&contenido);
    let selector_manager = scraper::Selector::parse(".download-manager a").expect("no se ha podido crear el selector de .download-manager");

    let mut extension_url_descarga: HashMap<String,String> = HashMap::new(); 

    for elemento in html.select(&selector_manager) {
        match elemento.value().attr("href") {
            None => {
                let mensaje_error = format!("ERROR: no se ha podido encontrar el enlace de descarga para" );
                return Err(mensaje_error);
            }
            Some(href) => {
               extension_url_descarga.insert(extension.to_string(), href.to_string());
            }
        }
    }

    Ok(extension_url_descarga)
}


impl Manejador {
    fn recuperar_extension(&mut self){   
        let argumento = &self.extensiones;
        if argumento.len() == 1 {
            panic!("no se ha incorporado ningún argumento con las extensiones deseadas");
        }
        let argumento_busqueda = argumento[argumento.len() - 1].clone();
    
        let argumento_extensiones:Vec<&str> = argumento_busqueda.split(",").collect();
        let mut extensiones = Vec::new();
        for ext in argumento_extensiones {
            let ext_minusculas = ext.to_ascii_lowercase();
            if ext_minusculas.starts_with(".") {
                extensiones.push(ext_minusculas.replace(".", ""));
                continue;
            }
            extensiones.push(ext_minusculas);
        }
        self.extensiones = extensiones;
    }

    async fn obtener_extensiones_validas(&mut self){
        let peticion = self.cliente.get("https://www.dwsamplefiles.com/post-sitemap.xml");
        let respuesta = peticion.send().await.expect("error crítico al enviar la peticion para analizar el sitemap");
        if respuesta.status().as_u16() != 200 {
            panic!("error crítico: la petición para analizar el sitemap ha devuelto un status incorrecto: {}", respuesta.status().as_u16());
        }
        
        let contenido = respuesta.text().await.expect("error crítico al obtener el contenido de la respuesta del sitemap");
        let html = scraper::Html::parse_document(&contenido);
        let selector = scraper::Selector::parse("loc").expect("no se ha podido crear el selector de loc");
    
        let mut info_extensiones: HashMap<String,String> = HashMap::new();
        let mut coincidencias = false;
        for elemento in html.select(&selector) {
            let texto_url = elemento.text().collect::<Vec<_>>().join(" ");
            if !texto_url.starts_with("https://www.dwsamplefiles.com/download-") {
                continue
            }
            let r = regex::Regex::new(r"https://www\.dwsamplefiles\.com/download-(.*?)-.*").expect("no se ha podido crear la expresión regular");
            let ext = r.replace_all(&texto_url, "$1").to_string();
            info_extensiones.insert(ext, texto_url);
        }
        let mut extensiones_validas: HashMap<String,String> = HashMap::new();
        for extension in self.extensiones.clone() {
            if !info_extensiones.contains_key(&extension) {
                let mensaje_error = format!("ADVERTENCIA: la extensión {} no está aceptada y no se podrá crear un archivo de este tipo", extension);
                println!("{}", mensaje_error)
            } else {
                coincidencias = true;
                let info_extension = info_extensiones.get(&extension).unwrap();
                extensiones_validas.insert(extension, info_extension.to_string());
            }
        }
    
        if !coincidencias {
            panic!("las extensiones introducidas no son válidas")
        }
    
        self.extensiones_verificadas = extensiones_validas;
    }
    
    async fn buscar_url_descarga(&mut self) {
        
        let mut futuros = Vec::new();
        
        for (extension, url) in self.extensiones_verificadas.clone() {
            futuros.push(obtener_url_descarga(&self.cliente, extension.clone(), url.clone()))
        }
        
        let mut url_descargas: HashMap<String, String> = HashMap::new();
        let mut coincidencias = false;
        match try_join_all(futuros).await {
            Err(error) => {
                eprintln!("{error}")
            }
            Ok(ok) => {
                coincidencias = true;
                for valor in ok {
                    url_descargas.extend(valor);
                }
            }
        }
        if !coincidencias {
            process::exit(1)
        }
        self.descargas = url_descargas;
    }

    async fn descargar_archivos(self) {

        let mut futuros = Vec::new();
    
        for (extension, url) in self.descargas {
            futuros.push(descargar(&self.cliente, extension, url))
        }
    
        match try_join_all(futuros).await {
            Err(error) => {
                eprintln!("{error}")
            }
            Ok(archivos_creados) => {
                for archivo in archivos_creados {
                    println!("ÉXITO: se ha creado el archivo {archivo}")
                }
            }
        }
    
    }
}

#[tokio::main] 
async fn main() {
    let mut manejador = Manejador{
        cliente: reqwest::Client::new(),
        extensiones: std::env::args().collect(),
        extensiones_verificadas: HashMap::new(),
        descargas: HashMap::new(),
    };
    manejador.recuperar_extension();
    manejador.obtener_extensiones_validas().await;
    manejador.buscar_url_descarga().await;
    manejador.descargar_archivos().await;
}
