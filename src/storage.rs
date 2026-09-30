//! Keeps the Wi-Fi credentials in flash, in the first sector of the `nvs` partition.
//!
//! Nothing else uses NVS here (esp-radio runs with NVS disabled on bare metal), and the partition
//! survives re-flashing unless the flasher is told to erase the whole chip.

use esp_bootloader_esp_idf::partitions::{
    self, DataPartitionSubType, FlashRegion, PARTITION_TABLE_MAX_LEN, PartitionType,
};
use esp_storage::FlashStorage;

use crate::credentials::{self, Credentials, RECORD_LEN};

fn nvs<'a, 'd>(flash: &'a mut FlashStorage<'d>) -> Result<FlashRegion<'a, 'd>, &'static str> {
    let mut table = [0u8; PARTITION_TABLE_MAX_LEN];
    let table = partitions::read_partition_table(flash, &mut table)
        .map_err(|_| "can't read partition table")?;
    let entry = table
        .find_partition(PartitionType::Data(DataPartitionSubType::Nvs))
        .map_err(|_| "bad partition table")?
        .ok_or("no nvs partition")?;
    Ok(entry.as_flash_region(flash))
}

pub fn load(flash: &mut FlashStorage<'_>) -> Option<Credentials> {
    let mut region = nvs(flash).ok()?;
    let mut record = [0u8; RECORD_LEN];
    region.read(0, &mut record).ok()?;
    credentials::decode(&record)
}

pub fn save(flash: &mut FlashStorage<'_>, credentials: &Credentials) -> Result<(), &'static str> {
    let record = credentials::encode(credentials);
    let mut region = nvs(flash)?;
    region
        .erase(0, FlashStorage::SECTOR_SIZE)
        .map_err(|_| "flash erase failed")?;
    region.write(0, &record).map_err(|_| "flash write failed")?;

    let mut check = [0u8; RECORD_LEN];
    region
        .read(0, &mut check)
        .map_err(|_| "flash read failed")?;
    if check != record {
        return Err("flash verify failed");
    }
    Ok(())
}
