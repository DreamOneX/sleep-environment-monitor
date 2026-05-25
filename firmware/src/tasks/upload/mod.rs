use core::fmt::{self, Write};

use crate::{
    config,
    types::{Measurement, TimeStatus},
};

include!("types.rs");
include!("json.rs");
include!("http.rs");
include!("parse.rs");
include!("time.rs");
include!("runtime.rs");

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::ErrorFlags;

    fn complete_measurement() -> Measurement {
        Measurement {
            uptime_ms: 1234,
            temperature_c: Some(21.5),
            humidity_percent: Some(45.25),
            lux: Some(9.75),
            mic_mean: 2048.0,
            mic_rms: 10.5,
            mic_peak: 99.0,
            mic_db_rel: 20.4,
            mic_clip_count: 2,
            error_flags: ErrorFlags::SHT40 | ErrorFlags::UPLOAD,
        }
    }

    fn encode_fields_to_str<'a>(
        m: &Measurement,
        out: &'a mut [u8],
    ) -> Result<&'a str, EncodeError> {
        let len = measurement_to_json_fields(m, out)?;
        Ok(core::str::from_utf8(&out[..len]).unwrap())
    }

    #[test]
    fn measurement_fields_encode_json_members() {
        let mut out = [0_u8; 256];

        assert_eq!(
            encode_fields_to_str(&complete_measurement(), &mut out).unwrap(),
            "\"uptime_ms\":1234,\"temperature_c\":21.5,\"humidity_percent\":45.25,\"lux\":9.75,\"mic_mean\":2048,\"mic_rms\":10.5,\"mic_peak\":99,\"mic_db_rel\":20.4,\"mic_clip_count\":2,\"error_flags\":17"
        );
    }

    #[test]
    fn missing_values_encode_as_json_null() {
        let mut measurement = complete_measurement();
        let mut out = [0_u8; 256];
        measurement.temperature_c = None;
        measurement.humidity_percent = None;
        measurement.lux = None;

        assert!(
            encode_fields_to_str(&measurement, &mut out)
                .unwrap()
                .contains("\"temperature_c\":null,\"humidity_percent\":null,\"lux\":null")
        );
    }

    #[test]
    fn json_payload_includes_sequence_and_wall_clock_when_synced() {
        let mut fields = [0_u8; 256];
        let fields_len = measurement_to_json_fields(&complete_measurement(), &mut fields).unwrap();
        let mut payload = [0_u8; 512];
        let payload_len = build_measurement_json(
            "device-1",
            7,
            &fields[..fields_len],
            TimestampSelection {
                status: TimeStatus::WallClockSynced,
                wall_clock_unix_ms: Some(1_700_000_000_000),
            },
            &mut payload,
        )
        .unwrap();
        let payload = core::str::from_utf8(&payload[..payload_len]).unwrap();

        assert!(payload.starts_with("{\"schema_version\":1,\"device_id\":\"device-1\""));
        assert!(payload.contains("\"sequence\":7"));
        assert!(payload.contains("\"time_status\":\"wall_clock_synced\""));
        assert!(payload.contains("\"wall_clock_unix_ms\":1700000000000"));
        assert!(payload.contains("\"uptime_ms\":1234"));
    }

    #[test]
    fn json_payload_omits_wall_clock_when_unknown() {
        let fields = b"\"uptime_ms\":42,\"temperature_c\":null";
        let mut payload = [0_u8; 256];
        let payload_len = build_measurement_json(
            "device-1",
            1,
            fields,
            TimestampSelection {
                status: TimeStatus::UptimeOnly,
                wall_clock_unix_ms: None,
            },
            &mut payload,
        )
        .unwrap();
        let payload = core::str::from_utf8(&payload[..payload_len]).unwrap();

        assert!(payload.contains("\"time_status\":\"uptime_only\""));
        assert!(!payload.contains("wall_clock_unix_ms"));
    }

    #[test]
    fn http_post_request_wraps_json_body() {
        let mut request = [0_u8; 512];
        let request_len = build_http_request(
            "POST",
            "10.133.56.218:8080",
            "/api/v1/measurements",
            Some("application/json"),
            b"{\"ok\":true}",
            &mut request,
        )
        .unwrap();
        let request = core::str::from_utf8(&request[..request_len]).unwrap();

        assert!(request.starts_with("POST /api/v1/measurements HTTP/1.1\r\n"));
        assert!(request.contains("Host: 10.133.56.218:8080\r\n"));
        assert!(request.contains("Content-Type: application/json\r\n"));
        assert!(request.contains("Content-Length: 11\r\n"));
        assert!(request.ends_with("{\"ok\":true}"));
    }

    #[test]
    fn http_response_class_accepts_2xx_only() {
        assert_eq!(
            http_response_class(b"HTTP/1.1 204 No Content\r\n\r\n"),
            ResponseClass::Success
        );
        assert_eq!(
            http_response_class(b"HTTP/1.1 500 Internal Server Error\r\n\r\n"),
            ResponseClass::HttpFailure(500)
        );
        assert_eq!(
            http_response_class(b"HTTP/1.1 302 Found\r\n\r\n"),
            ResponseClass::HttpFailure(302)
        );
        assert_eq!(
            http_response_class(b"bad response"),
            ResponseClass::Malformed
        );
    }

    #[test]
    fn http_response_total_len_uses_content_length() {
        assert_eq!(
            http_response_total_len(
                b"HTTP/1.1 200 OK\r\nContent-Length: 13\r\n\r\n{\"unix_ms\":1}"
            ),
            Some(52)
        );
        assert_eq!(
            http_response_total_len(b"HTTP/1.1 204 No Content\r\nContent-Length: 0\r\n\r\n"),
            Some(46)
        );
        assert_eq!(
            http_response_total_len(b"HTTP/1.1 200 OK\r\ncontent-length: 2\r\n\r\n{}"),
            Some(40)
        );
        assert_eq!(http_response_total_len(b"HTTP/1.1 200 OK\r\n\r\n{}"), None);
    }

    #[test]
    fn endpoint_resolution_prefers_provisioned_then_discovered_then_static() {
        let static_fallback = Endpoint::static_fallback();
        let discovered = Endpoint {
            ipv4: [192, 168, 1, 10],
            port: 8080,
            source: EndpointSource::Discovered,
        };
        let provisioned = Endpoint {
            ipv4: [10, 0, 0, 2],
            port: 9000,
            source: EndpointSource::Provisioned,
        };

        assert_eq!(
            resolve_endpoint(EndpointCandidates {
                provisioned: Some(provisioned),
                discovered: Some(discovered),
                static_fallback,
            }),
            provisioned
        );
        assert_eq!(
            resolve_endpoint(EndpointCandidates {
                provisioned: None,
                discovered: Some(discovered),
                static_fallback,
            }),
            discovered
        );
        assert_eq!(
            resolve_endpoint(EndpointCandidates::fallback_only()),
            static_fallback
        );
    }

    #[test]
    fn discovery_response_parses_endpoint() {
        let endpoint = parse_discovery_endpoint(
            br#"{"host":"192.168.1.44","port":8080,"api_base":"/api/v1"}"#,
        )
        .unwrap();

        assert_eq!(endpoint.ipv4, [192, 168, 1, 44]);
        assert_eq!(endpoint.port, 8080);
        assert_eq!(endpoint.source, EndpointSource::Discovered);
    }

    #[test]
    fn rest_time_response_parses_unix_ms() {
        assert_eq!(
            parse_rest_time_unix_ms(
                b"HTTP/1.1 200 OK\r\n\r\n{\"unix_ms\":1700000000123,\"source\":\"server\"}"
            ),
            Ok(1_700_000_000_123)
        );
    }

    #[test]
    fn sntp_response_parses_transmit_timestamp() {
        let mut packet = [0_u8; 48];
        packet[0] = 0b00_100_100;
        let seconds = config::upload::NTP_UNIX_EPOCH_DELTA_SECS + 1_700_000_000;
        packet[40..44].copy_from_slice(&(seconds as u32).to_be_bytes());
        packet[44..48].copy_from_slice(&0x8000_0000_u32.to_be_bytes());

        assert_eq!(parse_sntp_unix_ms(&packet), Ok(1_700_000_000_500));
    }

    #[test]
    fn timestamp_selection_uses_wall_clock_when_available() {
        let sync = TimeSyncState::new(1_000, 1_700_000_000_000);

        assert_eq!(
            select_timestamp(Some(sync), 1_250),
            TimestampSelection {
                status: TimeStatus::WallClockSynced,
                wall_clock_unix_ms: Some(1_700_000_000_250),
            }
        );
        assert_eq!(
            select_timestamp(None, 1_250),
            TimestampSelection {
                status: TimeStatus::UptimeOnly,
                wall_clock_unix_ms: None,
            }
        );
    }

    #[test]
    fn recovered_payloads_do_not_use_current_boot_time_sync() {
        let sync = TimeSyncState::new(1_000, 1_700_000_000_000);

        assert_eq!(
            select_payload_timestamp(false, Some(sync), 1_250),
            TimestampSelection {
                status: TimeStatus::UptimeOnly,
                wall_clock_unix_ms: None,
            }
        );
        assert_eq!(
            select_payload_timestamp(true, Some(sync), 1_250),
            TimestampSelection {
                status: TimeStatus::WallClockSynced,
                wall_clock_unix_ms: Some(1_700_000_000_250),
            }
        );
    }
}
