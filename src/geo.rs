//! Geofence math: great-circle distance (haversine), containment, and
//! closest-aircraft selection.

use crate::model::Aircraft;

/// Mean Earth radius in km.
const EARTH_RADIUS_KM: f64 = 6371.0;

/// Nautical miles per kilometer.
const NM_PER_KM: f64 = 1.0 / 1.852;

#[derive(Debug, Clone, Copy)]
pub struct Geofence {
    pub latitude: f64,
    pub longitude: f64,
    pub radius_km: f64,
}

impl Geofence {
    pub fn new(latitude: f64, longitude: f64, radius_km: f64) -> Self {
        Self {
            latitude,
            longitude,
            radius_km,
        }
    }

    /// Great-circle distance from the fence center to a point, in km.
    pub fn distance_km(&self, lat: f64, lon: f64) -> f64 {
        haversine_km(self.latitude, self.longitude, lat, lon)
    }

    pub fn contains(&self, lat: f64, lon: f64) -> bool {
        self.distance_km(lat, lon) <= self.radius_km
    }

    /// The fence radius in nautical miles, as required by the position API.
    /// Clamped to at least 1 NM so a tiny config radius can't produce an
    /// invalid zero-radius API request.
    pub fn radius_nm(&self) -> u32 {
        (self.radius_km * NM_PER_KM).round().max(1.0) as u32
    }
}

/// Great-circle distance between two coordinates, in km.
pub fn haversine_km(lat1: f64, lon1: f64, lat2: f64, lon2: f64) -> f64 {
    let dlat = (lat2 - lat1).to_radians();
    let dlon = (lon2 - lon1).to_radians();
    let a = (dlat / 2.0).sin().powi(2)
        + lat1.to_radians().cos() * lat2.to_radians().cos() * (dlon / 2.0).sin().powi(2);
    2.0 * EARTH_RADIUS_KM * a.sqrt().asin()
}

/// The aircraft closest to the fence center, or `None` for an empty slice.
pub fn closest<'a>(fence: &Geofence, aircraft: &'a [Aircraft]) -> Option<&'a Aircraft> {
    aircraft
        .iter()
        .min_by(|a, b| {
            fence
                .distance_km(a.lat, a.lon)
                .total_cmp(&fence.distance_km(b.lat, b.lon))
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Home coordinates and Athens International Airport (ATH).
    const HOME_LAT: f64 = 38.02260886547327;
    const HOME_LON: f64 = 24.005892241482684;
    const ATH_LAT: f64 = 37.9364;
    const ATH_LON: f64 = 23.9445;

    #[test]
    fn haversine_home_to_athens_airport() {
        let d = haversine_km(HOME_LAT, HOME_LON, ATH_LAT, ATH_LON);
        // Roughly 11 km by inspection; assert a tolerance band around it.
        assert!(
            (10.5..=11.5).contains(&d),
            "expected ~11 km, got {d}"
        );
    }

    #[test]
    fn haversine_zero_distance_and_antipode() {
        assert_eq!(haversine_km(HOME_LAT, HOME_LON, HOME_LAT, HOME_LON), 0.0);
        let d = haversine_km(0.0, 0.0, 0.0, 180.0);
        assert!((20000.0..=20040.0).contains(&d), "got {d}");
    }

    #[test]
    fn fence_radius_converts_to_nm() {
        let fence = Geofence::new(HOME_LAT, HOME_LON, 30.0);
        assert_eq!(fence.radius_nm(), 16); // 30 km / 1.852 ≈ 16.2 NM

        let tiny = Geofence::new(HOME_LAT, HOME_LON, 0.5);
        assert_eq!(tiny.radius_nm(), 1); // clamped, never 0
    }

    #[test]
    fn fence_contains_points_in_and_out() {
        let fence = Geofence::new(HOME_LAT, HOME_LON, 30.0);
        assert!(fence.contains(HOME_LAT, HOME_LON));
        assert!(fence.contains(ATH_LAT, ATH_LON)); // ~11 km away
        assert!(!fence.contains(38.9, 24.8)); // well over 100 km away
    }

    #[test]
    fn closest_picks_nearest_aircraft() {
        let mut near = Aircraft {
            hex: "111111".into(),
            flight: None,
            lat: HOME_LAT + 0.01,
            lon: HOME_LON,
            alt_baro: None,
            gs: None,
            baro_rate: None,
            track: None,
            r: None,
            t: None,
            seen_pos: None,
        };
        let far = Aircraft {
            hex: "222222".into(),
            lat: ATH_LAT,
            lon: ATH_LON,
            ..near.clone()
        };
        near.hex = "111111".into();
        let fence = Geofence::new(HOME_LAT, HOME_LON, 30.0);
        assert_eq!(closest(&fence, &[far.clone(), near.clone()]).unwrap().hex, "111111");
        assert_eq!(closest(&fence, std::slice::from_ref(&far)).unwrap().hex, "222222");
        assert!(closest(&fence, &[]).is_none());
    }
}
