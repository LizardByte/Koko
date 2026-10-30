// Koko's narrow C ABI for TSDuck. Native objects never cross the Rust boundary.
#include "tsDuckContext.h"
#include "tsTuner.h"
#include "tsModulationArgs.h"
#include "tsArgs.h"
#include "tsHFBand.h"
#include "tsNullReport.h"
#include "tsCOM.h"
#include "tsTSScanner.h"
#include "tsTableHandlerInterface.h"
#include "tsSectionDemux.h"
#include "tsBinaryTable.h"
#include "tsTSPacket.h"
#include "tsMGT.h"
#include "tsxmlDocument.h"
#include "tsxmlElement.h"
#include <cstring>
#include <memory>
#include <set>

#ifdef _WIN32
#define KOKO_EXPORT extern "C" __declspec(dllexport)
#else
#define KOKO_EXPORT extern "C" __attribute__((visibility("default")))
#endif

namespace {
    // Caller releases returned strings using koko_tv_free, in the same C++ runtime.
    char* copy(const ts::UString& text) {
        const auto utf8 = text.toUTF8();
        auto result = new char[utf8.size() + 1];
        std::memcpy(result, utf8.c_str(), utf8.size() + 1);
        return result;
    }

    struct Receiver {
        ts::COM com {NULLREP};
        ts::DuckContext duck {&NULLREP};
        ts::Tuner tuner {duck};
    };

    bool modulation(ts::DuckContext& duck, ts::ModulationArgs& tuning, const char* options) {
        ts::Args args(u"", u"", ts::Args::NO_EXIT_ON_ERROR | ts::Args::NO_EXIT_ON_HELP | ts::Args::NO_EXIT_ON_VERSION | ts::Args::NO_HELP);
        args.delegateReport(&NULLREP);
        tuning.defineArgs(args, false);
        return args.analyze(u"koko " + ts::UString::FromUTF8(options), false) && tuning.loadArgs(duck, args);
    }

    class GuideDecoder : public ts::TableHandlerInterface {
    public:
        ts::DuckContext duck {&NULLREP};
        ts::SectionDemux demux {duck, this};
        ts::xml::Document document {NULLREP};
        ts::xml::Element* root = document.initialize(u"tsduck");
        std::set<ts::UString> seen;
        size_t size = 0;

        GuideDecoder() { demux.addPID(0x11); demux.addPID(0x12); demux.addPID(0x1FFB); }
        void handleTable(ts::SectionDemux&, const ts::BinaryTable& table) override {
            const auto tid = table.tableId();
            if (tid == ts::TID_MGT) {
                ts::MGT mgt(duck, table);
                if (mgt.isValid()) {
                    for (const auto& [index, entry] : mgt.tables) {
                        if ((entry.table_type >= 0x100 && entry.table_type <= 0x17F) ||
                            (entry.table_type >= 0x200 && entry.table_type <= 0x27F)) demux.addPID(entry.table_type_PID);
                    }
                }
            }
            if (size >= 32 * 1024 * 1024 || seen.size() >= 20000) return;
            if (!(tid == 0x42 || tid == 0x46 || (tid >= 0x4E && tid <= 0x6F) || (tid >= 0xC8 && tid <= 0xCD))) return;
            ts::xml::Document temp(NULLREP);
            auto element = temp.initialize(u"tsduck");
            if (table.toXML(duck, element) == nullptr) return;
            const auto text = temp.toString();
            if (seen.insert(text).second) {
                size += text.size() * 3;
                if (size < 32 * 1024 * 1024) table.toXML(duck, root);
            }
        }
    };
}

KOKO_EXPORT unsigned koko_tv_abi() { return 2; }
KOKO_EXPORT void koko_tv_free(char* value) { delete[] value; }

KOKO_EXPORT char* koko_tv_devices() noexcept {
    try {
        ts::COM com(NULLREP);
        ts::DuckContext duck(&NULLREP);
        ts::TunerPtrVector tuners;
        ts::TunerBase::GetAllTuners(duck, tuners);
        ts::xml::Document doc(NULLREP);
        auto root = doc.initialize(u"tuners");
        for (const auto& tuner : tuners) {
            auto node = root->addElement(u"tuner");
            // BDA's stable device path distinguishes identical tuner filter names. Linux opens
            // adapter/frontend names, rather than the frontend node returned by devicePath().
#if defined(TS_WINDOWS)
            node->setAttribute(u"id", tuner->devicePath().empty() ? tuner->deviceName() : tuner->devicePath());
#else
            node->setAttribute(u"id", tuner->deviceName());
#endif
            node->setAttribute(u"name", tuner->deviceName());
            node->setAttribute(u"info", tuner->deviceInfo());
        }
        return copy(doc.toString());
    } catch (...) { return nullptr; }
}

KOKO_EXPORT char* koko_tv_regions() noexcept {
    try {
        ts::xml::Document doc(NULLREP);
        auto root = doc.initialize(u"regions");
        for (const auto& region : ts::HFBand::GetAllRegions(NULLREP)) {
            root->addElement(u"region")->setAttribute(u"name", region);
        }
        return copy(doc.toString());
    } catch (...) { return nullptr; }
}

// All methods on a Receiver, including destruction, run on its creating worker thread.
KOKO_EXPORT void* koko_tv_open(const char* device, const char* options) noexcept {
    try {
        auto receiver = std::make_unique<Receiver>();
        ts::ModulationArgs tuning;
        receiver->tuner.setSignalTimeout(cn::milliseconds(3000));
        if (!modulation(receiver->duck, tuning, options) || !receiver->tuner.open(ts::UString::FromUTF8(device), false)) return nullptr;
        receiver->tuner.setReceiveTimeout(cn::milliseconds(1000));
        if (!receiver->tuner.tune(tuning) || !receiver->tuner.start()) return nullptr;
        return receiver.release();
    } catch (...) { return nullptr; }
}

KOKO_EXPORT size_t koko_tv_read(void* handle, uint8_t* bytes, size_t packets) noexcept {
    try {
        if (packets > 4096) return 0;
        std::vector<ts::TSPacket> buffer(packets);
        const auto length = static_cast<Receiver*>(handle)->tuner.receive(buffer.data(), packets) * ts::PKT_SIZE;
        std::memcpy(bytes, buffer.data(), length);
        return length;
    } catch (...) { return 0; }
}
KOKO_EXPORT void koko_tv_close(void* handle) noexcept { try { delete static_cast<Receiver*>(handle); } catch (...) {} }

KOKO_EXPORT char* koko_tv_decode(const uint8_t* bytes, size_t length) noexcept {
    try {
        GuideDecoder decoder;
        // Find MPEG-TS alignment using three consecutive sync bytes, not an isolated payload byte.
        size_t offset = 0;
        for (; offset + 2 * ts::PKT_SIZE < length; ++offset) {
            if (bytes[offset] == 0x47 && bytes[offset + ts::PKT_SIZE] == 0x47 && bytes[offset + 2 * ts::PKT_SIZE] == 0x47) break;
        }
        for (; offset + ts::PKT_SIZE <= length; offset += ts::PKT_SIZE) {
            ts::TSPacket packet;
            std::memcpy(packet.b, bytes + offset, ts::PKT_SIZE);
            decoder.demux.feedPacket(packet);
        }
        decoder.demux.packAndFlushSections();
        return copy(decoder.document.toString());
    } catch (...) { return nullptr; }
}

// Scan terrestrial bands using the same BDA/DVB driver interface used for capture.
KOKO_EXPORT char* koko_tv_scan(const char* device, const char* system, const char* region, const char* extra,
                             bool (*cancelled)(void*), void* context) noexcept {
    try {
        Receiver receiver;
        if (!receiver.tuner.open(ts::UString::FromUTF8(device), false)) return nullptr;
        receiver.tuner.setSignalTimeout(cn::milliseconds(1500));
        receiver.tuner.setSignalTimeoutSilent(true);
        receiver.tuner.setReceiveTimeout(cn::milliseconds(1000));
        ts::xml::Document doc(NULLREP);
        auto root = doc.initialize(u"channels");
        for (const auto& band_name : {u"VHF", u"UHF"}) {
            const auto band = ts::HFBand::GetBand(ts::UString::FromUTF8(region), band_name, NULLREP);
            if (band == nullptr) continue;
            for (uint32_t channel = band->firstChannel(); channel <= band->lastChannel(); ++channel) {
                if (cancelled(context)) return nullptr;
                const auto frequency = band->frequency(channel);
                if (frequency == 0) continue;
                ts::ModulationArgs tuning;
                const auto options = ts::UString::Format(u"--delivery-system %s --frequency %d %s", ts::UString::FromUTF8(system), frequency, ts::UString::FromUTF8(extra));
                if (!modulation(receiver.duck, tuning, options.toUTF8().c_str())) return nullptr;
                if (!receiver.tuner.tune(tuning)) continue;
                // TSScanner starts and stops reception itself after collecting PAT/SDT/VCT.
                ts::TSScanner scanner(receiver.duck, receiver.tuner, cn::milliseconds(2500));
                ts::ServiceList services;
                if (!scanner.getServices(services)) continue;
                scanner.getTunerParameters(tuning);
                for (const auto& service : services) {
                    if (!service.hasId() || service.getCAControlled() || service.getHidden()) continue;
                    auto node = root->addElement(u"channel");
                    node->setIntAttribute(u"id", service.getId());
                    node->setIntAttribute(u"frequency", frequency);
                    node->setAttribute(u"name", service.getName());
                    node->setAttribute(u"tuning", tuning.toPluginOptions());
                    if (service.hasMajorIdATSC()) node->setAttribute(u"number", ts::UString::Format(u"%d.%d", service.getMajorIdATSC(), service.getMinorIdATSC()));
                    else if (service.hasLCN()) node->setIntAttribute(u"number", service.getLCN());
                }
            }
        }
        return copy(doc.toString());
    } catch (...) { return nullptr; }
}
