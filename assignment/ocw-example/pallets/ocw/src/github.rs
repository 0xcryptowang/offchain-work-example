pub mod github{
    
    use core::fmt;
    use sp_std::str;
    use serde::{Deserialize, Deserializer};
    use parity_scale_codec::{Decode, Encode};
    use sp_std::vec::Vec;
    use sp_runtime::{
		offchain as rt_offchain
    };

    const HTTP_REMOTE_REQUEST: &str = "https://api.github.com/orgs/substrate-developer-hub";
    const HTTP_HEADER_USER_AGENT: &str = "jimmychu0807";
	
	const FETCH_TIMEOUT_PERIOD: u64 = 3000; 

    #[derive(Deserialize, Encode, Decode, Default)]
	pub struct GithubInfo {
		#[serde(deserialize_with = "de_string_to_bytes")]
		login: Vec<u8>,
		#[serde(deserialize_with = "de_string_to_bytes")]
		blog: Vec<u8>,
		public_repos: u32,
	}
    
	impl fmt::Debug for GithubInfo {
		fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
			write!(
				f,
				"{{ login: {}, blog: {}, public_repos: {} }}",
				str::from_utf8(&self.login).map_err(|_| fmt::Error)?,
				str::from_utf8(&self.blog).map_err(|_| fmt::Error)?,
				&self.public_repos
				)
		}
	}
    
    #[derive(Debug)]
    pub enum Error {
		HttpRequestError,
        ConvertError
	}

    pub fn fetch_n_parse() -> Result<GithubInfo, Error> {
        let resp_bytes = fetch_from_remote().map_err(|e| {
            log::error!("fetch_from_remote error:{:?}", e);
            Error::HttpRequestError
        })?;
        let resp_str = str::from_utf8(&resp_bytes).map_err(|_| Error::ConvertError)?;
		log::info!("{}", resp_str);
        let gh_info: GithubInfo = serde_json::from_str(&resp_str).map_err(|_| Error::ConvertError)?;
		Ok(gh_info)
    }

    fn fetch_from_remote() -> Result<Vec<u8>, Error> {
        log::info!("sending request to: {}", HTTP_REMOTE_REQUEST);
        let request = rt_offchain::http::Request::get(HTTP_REMOTE_REQUEST);
        let timeout = sp_io::offchain::timestamp().add(rt_offchain::Duration::from_millis(FETCH_TIMEOUT_PERIOD));
        let pending = request.add_header("User-Agent", HTTP_HEADER_USER_AGENT).deadline(timeout).send().map_err(|_| Error::HttpRequestError)?;
        let response = pending.try_wait(timeout).map_err(|_| Error::HttpRequestError)?.map_err(|_| Error::HttpRequestError)?;
        if response.code != 200 {
            log::error!("Unexpected http request status code: {}", response.code);
            return Err(Error::HttpRequestError);
        }
        Ok(response.body().collect::<Vec<u8>>())
    }

    fn de_string_to_bytes<'de, D>(de: D) -> Result<Vec<u8>, D::Error> where D: Deserializer<'de> {
		let s: &str = Deserialize::deserialize(de)?;
		Ok(s.as_bytes().to_vec())
	}
}
