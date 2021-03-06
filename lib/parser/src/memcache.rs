//  rpc-perf - RPC Performance Testing
//  Copyright 2015 Twitter, Inc
//
//  Licensed under the Apache License, Version 2.0 (the "License");
//  you may not use this file except in compliance with the License.
//  You may obtain a copy of the License at
//
//      http://www.apache.org/licenses/LICENSE-2.0
//
//  Unless required by applicable law or agreed to in writing, software
//  distributed under the License is distributed on an "AS IS" BASIS,
//  WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
//  See the License for the specific language governing permissions and
//  limitations under the License.

pub use super::{Parse, ParsedResponse};

pub struct Response<'a> {
    pub response: &'a str,
}

impl<'a> Parse for Response<'a> {
    fn parse(&self) -> ParsedResponse {

        let mut lines: Vec<&str> = self.response.split("\r\n").collect();

        // expect an empty line from the split
        if lines[lines.len() - 1] == "" {
            let _ = lines.pop();
        } else {
            return ParsedResponse::Incomplete;
        }

        let tokens: Vec<&str> = lines[0].split_whitespace().collect();

        // one line responses can be special cased
        if lines.len() == 1 {
            // complete responses are short exactly 2 bytes for CRLF
            if lines[0].len() + 2 != self.response.len() {
                return ParsedResponse::Incomplete;
            }

            // special case 1 token responses
            if tokens.len() == 1 {
                match &*tokens[0] {
                    "OK" | "STORED" | "DELETED" => {
                        return ParsedResponse::Ok;
                    }
                    "END" | "EXISTS" | "NOT_FOUND" | "NOT_STORED" => {
                        return ParsedResponse::Miss;
                    }
                    "VALUE" => {
                        return ParsedResponse::Incomplete;
                    }
                    "ERROR" => {
                        return ParsedResponse::Error(self.response.to_owned());
                    }
                    _ => {}
                }
                // incr/decr give a numeric single token response
                if let Ok(_) = tokens[0].parse::<u64>() {
                    return ParsedResponse::Ok;
                }
            } else {
                match &*tokens[0] {
                    "VALUE" => {
                        return ParsedResponse::Incomplete;
                    }
                    "VERSION" => {
                        let v: String = tokens[1..tokens.len()].join(" ");
                        return ParsedResponse::Version(v);
                    }
                    "CLIENT_ERROR" | "SERVER_ERROR" => {
                        return ParsedResponse::Error(self.response.to_owned());
                    }
                    _ => {
                        return ParsedResponse::Unknown;
                    }
                }
            }
        } else {
            match &*tokens[0] {
                "VALUE" => {
                    if tokens.len() < 4 {
                        return ParsedResponse::Incomplete;
                    }
                    let bytes = tokens[3];
                    if tokens.len() == 5 {
                        match tokens[4].parse::<u64>() {
                            Ok(_) => {}
                            Err(_) => {
                                return ParsedResponse::Invalid;
                            }
                        }
                    }
                    match tokens[2].parse::<u32>() {
                        Ok(_) => {}
                        Err(_) => {
                            return ParsedResponse::Invalid;
                        }
                    }
                    if lines[lines.len() - 1] != "END" {
                        // END is always final line of complete response
                        return ParsedResponse::Incomplete;
                    }
                    let data = lines[1..lines.len() - 1].join("\r\n"); //reinsert any CRLF in data
                    match bytes.parse() {
                        Ok(b) => {
                            if data.len() == b {
                                // we have correct length data section
                                return ParsedResponse::Hit;
                            }
                            if data.len() > b {
                                // more data than in bytes field
                                return ParsedResponse::Invalid;
                            }
                        }
                        Err(_) => {
                            // bytes field failed to parse to usize
                            return ParsedResponse::Invalid;
                        }
                    }
                    return ParsedResponse::Incomplete;
                }
                _ => {
                    return ParsedResponse::Unknown;
                }
            }
        }
        ParsedResponse::Unknown
    }
}

#[cfg(test)]
mod tests {
    use super::{Parse, ParsedResponse, Response};

    #[test]
    fn test_parse_incomplete() {
        let r = Response { response: "0" };
        assert_eq!(r.parse(), ParsedResponse::Incomplete);

        let r = Response { response: "STOR" };
        assert_eq!(r.parse(), ParsedResponse::Incomplete);

        let r = Response { response: "STORED" };
        assert_eq!(r.parse(), ParsedResponse::Incomplete);

        let r = Response { response: "STORED\r" };
        assert_eq!(r.parse(), ParsedResponse::Incomplete);

        let r = Response { response: "VERSION " };
        assert_eq!(r.parse(), ParsedResponse::Incomplete);

        let r = Response { response: "VERSION 1.2.3" };
        assert_eq!(r.parse(), ParsedResponse::Incomplete);

        let r = Response { response: "VERSION 1.2.3\r" };
        assert_eq!(r.parse(), ParsedResponse::Incomplete);

        let r = Response { response: "CLIENT_ERROR" };
        assert_eq!(r.parse(), ParsedResponse::Incomplete);

        let r = Response { response: "SERVER_ERROR error msg" };
        assert_eq!(r.parse(), ParsedResponse::Incomplete);

        let r = Response { response: "VALUE key 0 1 0\r\n" };
        assert_eq!(r.parse(), ParsedResponse::Incomplete);

        let r = Response { response: "VALUE key 0 10\r\n0123456789\r\n" };
        assert_eq!(r.parse(), ParsedResponse::Incomplete);

        let r = Response { response: "VALUE key 0 10\r\n0123456789\r\nEND\r" };
        assert_eq!(r.parse(), ParsedResponse::Incomplete);

        let r = Response { response: "VALUE key 0 10\r\nEND\r\nEND\r\n\r\nEND" };
        assert_eq!(r.parse(), ParsedResponse::Incomplete);
    }

    #[test]
    fn test_parse_invalid() {
        let r = Response { response: "VALUE key 0 10\r\n0123456789ABCDEF\r\nEND\r\n" };
        assert_eq!(r.parse(), ParsedResponse::Invalid);

        let r = Response { response: "VALUE key 0 NaN\r\n0123456789ABCDEF\r\nEND\r\n" };
        assert_eq!(r.parse(), ParsedResponse::Invalid);

        let r = Response { response: "VALUE key NaN 10\r\n0123456789\r\nEND\r\n" };
        assert_eq!(r.parse(), ParsedResponse::Invalid);
    }

    #[test]
    fn test_parse_ok() {
        let r = Response { response: "OK\r\n" };
        assert_eq!(r.parse(), ParsedResponse::Ok);

        let r = Response { response: "STORED\r\n" };
        assert_eq!(r.parse(), ParsedResponse::Ok);

        let r = Response { response: "DELETED\r\n" };
        assert_eq!(r.parse(), ParsedResponse::Ok);

        let r = Response { response: "VALUE key 0 10\r\n0123456789\r\nEND\r\n" };
        assert_eq!(r.parse(), ParsedResponse::Hit);

        let r = Response { response: "VALUE key 0 10\r\n0123456789\r\nEND\r\n" };
        assert_eq!(r.parse(), ParsedResponse::Hit);

        let r = Response { response: "12345\r\n" };
        assert_eq!(r.parse(), ParsedResponse::Ok);
    }

    #[test]
    fn test_parse_error() {
        let r = Response { response: "ERROR\r\n" };
        assert_eq!(r.parse(), ParsedResponse::Error("ERROR\r\n".to_owned()));

        let r = Response { response: "CLIENT_ERROR some message\r\n" };
        assert_eq!(r.parse(),
                   ParsedResponse::Error("CLIENT_ERROR some message\r\n".to_owned()));

        let r = Response { response: "SERVER_ERROR some message\r\n" };
        assert_eq!(r.parse(),
                   ParsedResponse::Error("SERVER_ERROR some message\r\n".to_owned()));
    }

    #[test]
    fn test_parse_miss() {
        let r = Response { response: "EXISTS\r\n" };
        assert_eq!(r.parse(), ParsedResponse::Miss);

        let r = Response { response: "NOT_FOUND\r\n" };
        assert_eq!(r.parse(), ParsedResponse::Miss);

        let r = Response { response: "NOT_STORED\r\n" };
        assert_eq!(r.parse(), ParsedResponse::Miss);
    }

    #[test]
    fn test_parse_version() {
        let r = Response { response: "VERSION 1.2.3\r\n" };
        assert_eq!(r.parse(), ParsedResponse::Version("1.2.3".to_owned()));
    }
}
