pub mod dot{
    // 导入引用
    use sp_std::str;
    use serde::{Deserialize, Deserializer};
    use parity_scale_codec::{Decode, Encode};
    use sp_std::vec::Vec;
    use sp_runtime::{
		offchain as rt_offchain
    };
    use sp_arithmetic::per_things::Permill;

    
    // 定义常量
    const HTTP_REMOTE_REQUEST: &str = "https://api.coincap.io/v2/assets/polkadot";
    const FETCH_TIMEOUT_PERIOD: u64 = 3000; 

    
    // 定义结构体
    #[derive(Debug, Deserialize, Encode, Decode, Default)]
	struct HttpResponse {
		data: PriceInfo,
		timestamp: u64,
	}

    #[derive(Debug, Deserialize, Encode, Decode, Default)]
    pub struct PriceInfo{
        #[serde(deserialize_with = "de_string_to_bytes")]
		pub id: Vec<u8>,
        #[serde(deserialize_with = "de_string_to_bytes")]
        pub symbol: Vec<u8>,
        #[serde(deserialize_with = "de_string_to_bytes", rename = "priceUsd")]
        pub price_usd: Vec<u8>
    }


    // 定义异常
    #[derive(Debug)]
    pub enum Error {
		HttpRequestError,
        ConvertError
	}


    /// 转换并格式化请求结果
    pub fn fetch_dot_price_parse() -> Result<PriceInfo, Error> {
        let resp_bytes = fetch_dot_price().map_err(|e| {
            log::error!("fetch_dot_price error:{:?}", e);
            Error::HttpRequestError
        })?;
        log::info!("resp_bytes: {:?}", resp_bytes);
        let response_json: HttpResponse = serde_json::from_slice(&resp_bytes).map_err(|_| {Error::ConvertError})?;
        log::info!("response_json: {:?}", response_json);
        let price_info_json: PriceInfo = response_json.data;
        log::info!("price_info_json: {:?}", price_info_json);
        Ok(price_info_json)
    }


    /// 发送http请求api
    fn fetch_dot_price() -> Result<Vec<u8>, Error>{
        log::info!("sending request to: {}", HTTP_REMOTE_REQUEST);
        let request = rt_offchain::http::Request::get(HTTP_REMOTE_REQUEST);
        let timeout = sp_io::offchain::timestamp().add(rt_offchain::Duration::from_millis(FETCH_TIMEOUT_PERIOD));
        let pending = request.deadline(timeout).send().map_err(|_| Error::HttpRequestError)?;
        let response = pending.try_wait(timeout).map_err(|_| Error::HttpRequestError)?.map_err(|_| Error::HttpRequestError)?;
        if response.code != 200 {
            log::error!("Unexpected http request status code: {}", response.code);
            return Err(Error::HttpRequestError);
        }
        Ok(response.body().collect::<Vec<u8>>())
    }


    /// 序列化转换
    fn de_string_to_bytes<'de, D>(de: D) -> Result<Vec<u8>, D::Error> where D: Deserializer<'de> {
		let s: &str = Deserialize::deserialize(de)?;
		Ok(s.as_bytes().to_vec())
	}

    /// 格式化价格
    pub fn parse_price_usd(price_usd: Vec<u8>) ->  Option<(u64, Permill)>{
        let mut point_position:usize = 0;
        for item in price_usd.iter() {
            if *item == b'.' {
                break;
            }
            point_position += 1;
        }
        let  price_usd_integer = parse_price_usd_integer(price_usd.clone(),point_position);
        let  price_usd_permil = parse_price_usd_permil(price_usd,point_position);
        return Some((price_usd_integer, price_usd_permil));
    }
    
    /// 格式化小数点前整数
    fn parse_price_usd_integer(price_usd: Vec<u8>, point_position:usize) -> u64{
        let price_usd_integer = &price_usd[0..point_position];
        let price_usd_integer_str = str::from_utf8(&price_usd_integer).unwrap();
        let price_usd_fraction_integer: u64 = price_usd_integer_str.parse().unwrap();
        log::info!("{}", price_usd_fraction_integer);
        return price_usd_fraction_integer;
    }
    
    /// 格式化小数点后小数，注意溢出问题
    fn parse_price_usd_permil(price_usd: Vec<u8>, point_position:usize) -> Permill{
        let price_usd_len = price_usd.len();
        let price_usd_fraction = match price_usd_len >= point_position + 7 {
            true => &price_usd[point_position + 1..point_position + 7],
            false => &price_usd[point_position + 1..point_position],
        };
        let price_usd_fraction_str = str::from_utf8(&price_usd_fraction).unwrap();
        let price_usd_fraction_permil = Permill::from_parts(price_usd_fraction_str.parse().unwrap());
        log::info!("{:?}", price_usd_fraction_permil);
        return price_usd_fraction_permil;
    }
}