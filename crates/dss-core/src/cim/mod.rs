//! CIM exporter state — the UUID plumbing of Pascal `Common/ExportCIMXML.pas`
//! (`TCIMExporter`) and `General/NamedObject.pas` (PHASE8_PLAN WP8.6 step 6).
//!
//! This seeds the module home PORTING_PLAN assigns to the CIM XML exports:
//! WP8.6 lands the persistent hashed-UUID state (`UuidHash`/`UuidList`/
//! `UuidKeyList`) plus the four helpers `StartUuidList`/`GetHashedUuid`/
//! `GetDevUuid`/`AddHashedUuid` (and `WriteHashedUUIDs`/`FreeUuidList`), which
//! the `Uuids` command and `Export Uuids` drive. GAPS_PLAN WPG.18 (the CIM100
//! XML exporters) builds on **this exact storage shape** — do not reshape it.
//!
//! The hashed list **persists across commands** on the DSS context: the
//! `FreeUuidList` call after the CIM XML export is commented out upstream
//! ("deferred for UUID export", `ExportCIMXML.pas:4697`); `ExportUuids` frees
//! it in its `finally`, and `DoUuidsCmd` resets it via `StartUuidList`.

#[cfg(test)]
mod tests;

use crate::support::hashlist::HashList;

/// Pascal `TUuid` (an FPC `TGuid`). Stored as the canonical 16 bytes; rendered
/// via [`Uuid::to_dss_string`] = FPC `GuidToString` (braced, uppercase hex).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Uuid(uuid::Uuid);

impl Uuid {
    /// Pascal `CreateUUID4` (`NamedObject.pas:47`): a random version-4 UUID.
    /// Never oracle-pinnable — goldens preload every UUID they compare.
    pub fn create_v4() -> Self {
        Self(uuid::Uuid::new_v4())
    }

    /// Pascal `StringToUUID` (FPC `StringToGUID`): parse a `{…}`-braced (or
    /// bare hyphenated) UUID string. `None` where FPC raises `EConvertError`.
    pub fn parse(s: &str) -> Option<Self> {
        let t = s.trim();
        let t = t.strip_prefix('{').unwrap_or(t);
        let t = t.strip_suffix('}').unwrap_or(t);
        uuid::Uuid::try_parse(t).ok().map(Self)
    }

    /// Pascal `UUIDToString` (FPC `GuidToString`): `{XXXXXXXX-XXXX-XXXX-XXXX-
    /// XXXXXXXXXXXX}`, uppercase hex (the form `Export Uuids` prints).
    pub fn to_dss_string(&self) -> String {
        format!("{{{}}}", self.0.hyphenated().to_string().to_uppercase())
    }
}

/// Lazily-created UUID slot shared by every named object (`TNamedObject.pUuid`,
/// `NamedObject.pas:47-52`): `Get_UUID` creates a **random v4** on first read.
/// Free function (not a method) because the slot lives on plain structs
/// (`Circuit`, `Bus`, `DssObjData`).
pub fn get_or_create_uuid(slot: &mut Option<Uuid>) -> Uuid {
    *slot.get_or_insert_with(Uuid::create_v4)
}

/// Pascal `UuidChoice` (`ExportCIMXML.pas`): which device-UUID key family a
/// [`CimExporter::get_dev_uuid`] call belongs to. Transcribed in full (the key
/// prefixes are part of the persistent-list contract WPG.18 builds on); only
/// `Station`/`GeoRgn`/`SubGeoRgn` are constructed before WPG.18 lands.
#[allow(dead_code)] // the CIM XML exporters (GAPS_PLAN WPG.18) construct the rest
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UuidChoice {
    Bank,
    Wdg,
    XfCore,
    XfMesh,
    WdgInf,
    ScTest,
    OcTest,
    BaseV,
    LinePhase,
    LoadPhase,
    GenPhase,
    CapPhase,
    SolarPhase,
    BatteryPhase,
    XfLoc,
    LoadLoc,
    LineLoc,
    CapLoc,
    Topo,
    ReacLoc,
    SolarLoc,
    BatteryLoc,
    LoadResp,
    CIMVer,
    PosPt,
    CoordSys,
    TopoIsland,
    Station,
    GeoRgn,
    SubGeoRgn,
    ZData,
    OpLimV,
    OpLimI,
    OpLimT,
    FdrLoc,
    XfInfo,
    OpLimAHi,
    OpLimALo,
    OpLimBHi,
    OpLimBLo,
    MachLoc,
    SrcLoc,
    PVPanels,
    Battery,
    TankInfo,
    TapCtrl,
    PUZ,
    WirePos,
    NormAmps,
    EmergAmps,
    I1547NameplateData,
    I1547NameplateDataApplied,
    I1547Signal,
    I1547VoltVar,
    I1547WattVar,
    I1547ConstPF,
    I1547VoltWatt,
    I1547ConstQ,
    ECProfile,
}

impl UuidChoice {
    /// The exact key prefix `GetDevUuid` builds (`ExportCIMXML.pas:1002`,
    /// `case which of … key := '…='`).
    fn key_prefix(self) -> &'static str {
        use UuidChoice::*;
        match self {
            Bank => "Bank=",
            Wdg => "Wdg=",
            XfCore => "XfCore=",
            XfMesh => "XfMesh=",
            WdgInf => "WdgInf=",
            ScTest => "ScTest=",
            OcTest => "OcTest=",
            BaseV => "BaseV=",
            OpLimV => "OpLimV=",
            OpLimI => "OpLimI=",
            LinePhase => "LinePhase=",
            LoadPhase => "LoadPhase=",
            GenPhase => "GenPhase=",
            SolarPhase => "PVPhase=",
            BatteryPhase => "BattPhase=",
            CapPhase => "CapPhase=",
            XfLoc => "XfLoc=",
            LoadLoc => "LoadLoc=",
            LineLoc => "LineLoc=",
            ReacLoc => "ReacLoc=",
            CapLoc => "CapLoc=",
            Topo => "Topo=",
            SolarLoc => "SolarLoc=",
            BatteryLoc => "BatteryLoc=",
            LoadResp => "LoadResp=",
            CIMVer => "CIMVer=",
            ZData => "ZData=",
            PosPt => "PosPt=",
            CoordSys => "CoordSys=",
            TopoIsland => "TopoIsland=",
            OpLimT => "OpLimT=",
            Station => "Station=",
            GeoRgn => "GeoRgn=",
            SubGeoRgn => "SubGeoRgn=",
            FdrLoc => "FdrLoc=",
            XfInfo => "XfInfo=",
            OpLimAHi => "OpLimAHi=",
            OpLimALo => "OpLimALo=",
            OpLimBHi => "OpLimBHi=",
            OpLimBLo => "OpLimBLo=",
            MachLoc => "MachLoc=",
            SrcLoc => "SrcLoc=",
            PVPanels => "PVPanels=",
            Battery => "Battery=",
            TankInfo => "TankInfo=",
            TapCtrl => "TapCtrl=",
            PUZ => "PUZ=",
            WirePos => "WirePos=",
            NormAmps => "NormAmps=",
            EmergAmps => "EmergAmps=",
            I1547NameplateData => "INameplate=",
            I1547NameplateDataApplied => "IApplied=",
            I1547Signal => "ISignal=",
            I1547VoltVar => "IVVar=",
            I1547WattVar => "IWVar=",
            I1547ConstPF => "IPF=",
            I1547VoltWatt => "IVWatt=",
            I1547ConstQ => "IQ=",
            ECProfile => "ECP=",
        }
    }
}

/// The UUID-list half of Pascal `TCIMExporter`: `UuidHash` (a case-insensitive
/// `THashList`), `UuidList` (the values) and `UuidKeyList` (the original-case
/// keys `WriteHashedUUIDs` prints). One instance lives on the DSS context and
/// persists across commands.
#[derive(Default)]
pub struct CimExporter {
    /// `Some` iff the list is started (Pascal `assigned(UuidList)`); the
    /// initial `size` hint is a Pascal pre-allocation, not a capacity limit.
    uuid_hash: Option<HashList>,
    uuid_list: Vec<Uuid>,
    uuid_key_list: Vec<String>,
}

impl CimExporter {
    /// Pascal `TCIMExporter.StartUuidList` (`ExportCIMXML.pas:822`): (re)create
    /// the hashed-UUID list, dropping any previous content.
    pub fn start_uuid_list(&mut self, size: usize) {
        if self.uuid_hash.is_some() {
            self.free_uuid_list();
        }
        self.uuid_hash = Some(HashList::with_capacity(size));
    }

    /// Pascal `TCIMExporter.FreeUuidList` (`ExportCIMXML.pas:849`).
    pub fn free_uuid_list(&mut self) {
        self.uuid_hash = None;
        self.uuid_list.clear();
        self.uuid_key_list.clear();
    }

    /// Pascal `assigned(UuidList)` — whether the list is started
    /// (`DefaultCircuitUUIDs` starts it only when it is not).
    pub fn is_started(&self) -> bool {
        self.uuid_hash.is_some()
    }

    /// Pascal `TCIMExporterHelper.GetHashedUuid` (`ExportCIMXML.pas:952`):
    /// find-or-`CreateUUID4` for `key`. Starts the list if never started (the
    /// Pascal call sites always start it first; this keeps the port total).
    pub fn get_hashed_uuid(&mut self, key: &str) -> Uuid {
        let hash = self.uuid_hash.get_or_insert_with(HashList::new);
        match hash.find(key) {
            Some(r) => self.uuid_list[r],
            None => {
                hash.add(key);
                let u = Uuid::create_v4();
                self.uuid_list.push(u);
                self.uuid_key_list.push(key.to_string());
                u
            }
        }
    }

    /// Pascal `TCIMExporter.AddHashedUuid` (`ExportCIMXML.pas:977`): insert or
    /// overwrite `key`'s UUID from its string form. `Err` where FPC's
    /// `StringToUuid` raises `EConvertError` (an unparsable UUID).
    pub fn add_hashed_uuid(&mut self, key: &str, uuid_val: &str) -> Result<(), String> {
        let u = Uuid::parse(uuid_val)
            .ok_or_else(|| format!("\"{uuid_val}\" is not a valid UUID value."))?;
        let hash = self.uuid_hash.get_or_insert_with(HashList::new);
        match hash.find(key) {
            Some(r) => self.uuid_list[r] = u,
            None => {
                hash.add(key);
                self.uuid_list.push(u);
                self.uuid_key_list.push(key.to_string());
            }
        }
        Ok(())
    }

    /// Pascal `TCIMExporterHelper.GetDevUuid` (`ExportCIMXML.pas:1002`): the
    /// hashed UUID for key `<prefix><name>=<seq>` (temporary CIM objects with
    /// no DSS named-object counterpart).
    pub fn get_dev_uuid(&mut self, which: UuidChoice, name: &str, seq: i32) -> Uuid {
        let key = format!("{}{}={}", which.key_prefix(), name, seq);
        self.get_hashed_uuid(&key)
    }

    /// Pascal `TCIMExporter.WriteHashedUUIDs` (`ExportCIMXML.pas:1286`): one
    /// `<key> <UUID>` line per stored key, in insertion order (the Pascal loop
    /// breaks at the first empty `UuidKeyList` slot — our list holds only real
    /// keys).
    pub fn write_hashed_uuids(&self, out: &mut String) {
        for (key, u) in self.uuid_key_list.iter().zip(&self.uuid_list) {
            out.push_str(key);
            out.push(' ');
            out.push_str(&u.to_dss_string());
            out.push('\n');
        }
    }
}
