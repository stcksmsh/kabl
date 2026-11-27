// kabl-vst3-host <kabl.vst3> <midi-fixture.json>
//
// Loads kabl.vst3 the way a VST3 host does and checks what Ableton Live and REAPER rely on:
// the 18 parameters with their frozen ids, automatable, in their units; and MIDI controllers
// reaching kabl through IMidiMapping, which is the only way VST3 delivers CCs and pitch bend.
//
// The controller checks play the fixture state (the default patch plus the mod wheel routed
// to the oscillator's fine tune and CC 74 mapped to its base frequency) and measure the
// oscillator in the audio output: pitch bend and the mod wheel raise it, CC 74 moves it, and
// the sustain pedal holds a released note until the pedal is let go.
//
// Prints one PASS or FAIL line per check; the exit code is the number of failed checks
// (a setup failure exits with 100).

#include "public.sdk/source/common/memorystream.h"
#include "public.sdk/source/vst/hosting/eventlist.h"
#include "public.sdk/source/vst/hosting/hostclasses.h"
#include "public.sdk/source/vst/hosting/module.h"
#include "public.sdk/source/vst/hosting/parameterchanges.h"
#include "public.sdk/source/vst/hosting/plugprovider.h"
#include "public.sdk/source/vst/hosting/processdata.h"
#include "public.sdk/source/vst/utility/stringconvert.h"
#include "pluginterfaces/vst/ivstaudioprocessor.h"
#include "pluginterfaces/vst/ivstcomponent.h"
#include "pluginterfaces/vst/ivsteditcontroller.h"
#include "pluginterfaces/vst/ivstmidicontrollers.h"
#include "pluginterfaces/vst/ivstunits.h"

#include <algorithm>
#include <chrono>
#include <cmath>
#include <cstdint>
#include <cstdio>
#include <cstdlib>
#include <fstream>
#include <iterator>
#include <map>
#include <set>
#include <sstream>
#include <string>
#include <thread>
#include <vector>

using namespace Steinberg;
using namespace Steinberg::Vst;

namespace {

constexpr double kRate = 48000.0;
constexpr int32 kBlock = 512;

int failures = 0;

void check(bool ok, const std::string& what)
{
    std::printf("%s: %s\n", ok ? "PASS" : "FAIL", what.c_str());
    std::fflush(stdout);
    if (!ok)
        ++failures;
}

void require(bool ok, const std::string& what)
{
    if (!ok)
    {
        std::printf("SETUP FAILED: %s\n", what.c_str());
        std::exit(100);
    }
}

std::string utf8(const TChar* s) { return StringConvert::convert(s); }

// The frozen host parameter ids: nice-plug's hash of each string id
// (vendor/nice-plug/crates/nice-plug/src/wrapper/util.rs, hash_param_id), in list order.
struct Expected
{
    const char* key;
    ParamID id;
};
const Expected kParams[18] = {
    {"output_gain", 1322673149}, {"slot_1", 1248030256},  {"slot_2", 1248030257},
    {"slot_3", 1248030258},      {"slot_4", 1248030259},  {"slot_5", 1248030260},
    {"slot_6", 1248030261},      {"slot_7", 1248030262},  {"slot_8", 1248030263},
    {"slot_9", 1248030264},      {"slot_10", 34232320},   {"slot_11", 34232321},
    {"slot_12", 34232322},       {"slot_13", 34232323},   {"slot_14", 34232324},
    {"slot_15", 34232325},       {"slot_16", 34232326},   {"host_clock", 1371428535},
};

struct Host
{
    IPtr<IComponent> component;
    FUnknownPtr<IAudioProcessor> processor;
    HostProcessData data;
    EventList events {64};
    ParameterChanges changes {8};
    ProcessContext context {};
    std::vector<float> left;

    void note(bool on)
    {
        Event e {};
        e.busIndex = 0;
        e.sampleOffset = 0;
        if (on)
        {
            e.type = Event::kNoteOnEvent;
            e.noteOn.channel = 0;
            e.noteOn.pitch = 60;
            e.noteOn.velocity = 0.8f;
            e.noteOn.noteId = -1;
        }
        else
        {
            e.type = Event::kNoteOffEvent;
            e.noteOff.channel = 0;
            e.noteOff.pitch = 60;
            e.noteOff.velocity = 0.0f;
            e.noteOff.noteId = -1;
        }
        events.addEvent(e);
    }

    void send(ParamID id, ParamValue value)
    {
        int32 index = 0;
        auto* queue = changes.addParameterData(id, index);
        require(queue != nullptr, "parameter queue");
        queue->addPoint(0, value, index);
    }

    // Blocks of audio; the short sleep lets kabl's control thread (which applies CC mappings)
    // run between them, as it does between a real host's callbacks.
    void render(int blocks)
    {
        for (int b = 0; b < blocks; ++b)
        {
            data.numSamples = kBlock;
            data.inputEvents = &events;
            data.inputParameterChanges = &changes;
            data.processContext = &context;
            require(processor->process(data) == kResultOk, "IAudioProcessor::process");
            const float* out = data.outputs[0].channelBuffers32[0];
            left.insert(left.end(), out, out + kBlock);
            context.projectTimeSamples += kBlock;
            events.clear();
            changes.clearQueue();
            std::this_thread::sleep_for(std::chrono::milliseconds(2));
        }
    }

    std::vector<float> last(int blocks) const
    {
        return std::vector<float>(left.end() - blocks * kBlock, left.end());
    }
};

// Rising zero crossings with hysteresis, from the first to the last one.
double frequency(const std::vector<float>& s)
{
    float peak = 0;
    for (float v : s)
        peak = std::max(peak, std::fabs(v));
    const float threshold = 0.2f * peak;
    bool below = false;
    long first = -1, last = -1, count = 0;
    for (size_t i = 0; i < s.size(); ++i)
    {
        if (s[i] < -threshold)
            below = true;
        else if (below && s[i] > threshold)
        {
            below = false;
            if (first < 0)
                first = static_cast<long>(i);
            last = static_cast<long>(i);
            ++count;
        }
    }
    if (count < 2 || peak < 1e-3f)
        return 0;
    return (count - 1) * kRate / static_cast<double>(last - first);
}

double rms(const std::vector<float>& s)
{
    double sum = 0;
    for (float v : s)
        sum += static_cast<double>(v) * v;
    return std::sqrt(sum / static_cast<double>(s.size()));
}

std::string fmt(const char* format, double a, double b = 0)
{
    char buffer[160];
    std::snprintf(buffer, sizeof buffer, format, a, b);
    return buffer;
}

} // namespace

int main(int argc, char* argv[])
{
    if (argc != 3)
    {
        std::printf("usage: kabl-vst3-host <kabl.vst3> <midi-fixture.json>\n");
        return 100;
    }
    std::ifstream file(argv[2], std::ios::binary);
    require(file.good(), std::string("read ") + argv[2]);
    const std::string fixture((std::istreambuf_iterator<char>(file)), std::istreambuf_iterator<char>());

    std::string error;
    auto module = VST3::Hosting::Module::create(argv[1], error);
    require(module != nullptr, "load " + std::string(argv[1]) + ": " + error);
    auto hostApp = owned(new HostApplication());
    PluginContextFactory::instance().setPluginContext(hostApp);
    const auto& factory = module->getFactory();
    IPtr<PlugProvider> provider;
    for (auto& info : factory.classInfos())
    {
        if (info.category() == kVstAudioEffectClass)
        {
            provider = owned(new PlugProvider(factory, info, true));
            break;
        }
    }
    require(provider != nullptr, "an audio module class");
    require(provider->initialize(), "create and initialize the component and controller");
    IPtr<IComponent> component = provider->getComponentPtr();
    IPtr<IEditController> controller = provider->getControllerPtr();
    require(component && controller, "component and controller");

    // --- The MIDI controller parameters clap-wrapper adds (IMidiMapping) ---
    FUnknownPtr<IMidiMapping> mapping(controller);
    check(mapping, "the controller implements IMidiMapping");
    require(mapping, "IMidiMapping");
    std::set<ParamID> midiIds;
    for (int16 channel = 0; channel < 16; ++channel)
    {
        for (CtrlNumber cc = 0; cc < kCountCtrlNumber; ++cc)
        {
            ParamID id = kNoParamId;
            if (mapping->getMidiControllerAssignment(0, channel, cc, id) == kResultTrue)
                midiIds.insert(id);
        }
    }

    // --- kabl's own parameters: everything that is not a MIDI controller or program change ---
    std::vector<ParameterInfo> params;
    std::map<ParamID, ParameterInfo> byId;
    const int32 count = controller->getParameterCount();
    for (int32 i = 0; i < count; ++i)
    {
        ParameterInfo info {};
        require(controller->getParameterInfo(i, info) == kResultOk, "getParameterInfo");
        byId[info.id] = info;
        if (midiIds.count(info.id) == 0 && (info.flags & ParameterInfo::kIsProgramChange) == 0)
            params.push_back(info);
    }
    std::printf("%d parameters in all, %zu MIDI controller parameters\n", count, midiIds.size());
    for (auto& p : params)
        std::printf("  id %u '%s' flags 0x%x unit %d\n", p.id, utf8(p.title).c_str(), p.flags, p.unitId);
    check(params.size() == 18, fmt("kabl lists 18 parameters (found %.0f)", double(params.size())));
    bool ids = params.size() == 18;
    for (size_t i = 0; ids && i < 18; ++i)
        ids = params[i].id == kParams[i].id;
    check(ids, "the parameter ids are the frozen ones, in order");

    FUnknownPtr<IUnitInfo> unitInfo(controller);
    require(unitInfo, "the controller implements IUnitInfo");
    std::map<UnitID, UnitInfo> units;
    for (int32 i = 0; i < unitInfo->getUnitCount(); ++i)
    {
        UnitInfo u {};
        require(unitInfo->getUnitInfo(i, u) == kResultOk, "getUnitInfo");
        units[u.id] = u;
        std::printf("  unit %d '%s' parent %d\n", u.id, utf8(u.name).c_str(), u.parentUnitId);
    }
    for (size_t i = 0; i < params.size() && i < 18; ++i)
    {
        const auto& p = params[i];
        const std::string key = kParams[i].key;
        const std::string title = utf8(p.title);
        const bool visible = (p.flags & (ParameterInfo::kIsHidden | ParameterInfo::kIsReadOnly)) == 0;
        if (key != "host_clock")
            check((p.flags & ParameterInfo::kCanAutomate) && visible,
                  key + " '" + title + "' is automatable, visible and writable");
        if (key.rfind("slot_", 0) == 0)
        {
            const std::string want = "Automation " + key.substr(5);
            auto u = units.find(p.unitId);
            check(u != units.end() && utf8(u->second.name) == want &&
                      u->second.parentUnitId == kRootUnitId,
                  key + " is in unit '" + want + "' under the root unit");
        }
        else
        {
            check(p.unitId == kRootUnitId, key + " is in the root unit");
        }
    }

    ParamID wheel = kNoParamId, sustain = kNoParamId, bend = kNoParamId, cc74 = kNoParamId;
    const bool mapped = mapping->getMidiControllerAssignment(0, 0, kCtrlModWheel, wheel) == kResultTrue &&
                        mapping->getMidiControllerAssignment(0, 0, kCtrlSustainOnOff, sustain) == kResultTrue &&
                        mapping->getMidiControllerAssignment(0, 0, kPitchBend, bend) == kResultTrue &&
                        mapping->getMidiControllerAssignment(0, 0, 74, cc74) == kResultTrue;
    std::set<ParamID> distinct {wheel, sustain, bend, cc74};
    bool parameters = true;
    for (ParamID id : distinct)
        parameters = parameters && byId.count(id) == 1;
    check(mapped && distinct.size() == 4 && parameters,
          "CC 1, CC 64, pitch bend and CC 74 on channel 1 map to four distinct parameters");
    require(mapped && distinct.size() == 4 && parameters, "MIDI controller parameters");

    // --- Play the fixture ---
    MemoryStream state;
    const uint64_t length = fixture.size();
    unsigned char header[8];
    for (int i = 0; i < 8; ++i)
        header[i] = static_cast<unsigned char>(length >> (8 * i));
    require(state.write(header, 8, nullptr) == kResultOk &&
                state.write(const_cast<char*>(fixture.data()), static_cast<int32>(fixture.size()), nullptr) == kResultOk,
            "state stream");
    state.seek(0, IBStream::kIBSeekSet, nullptr);
    check(component->setState(&state) == kResultOk, "IComponent::setState loads the fixture");

    Host host;
    host.component = component;
    host.processor = FUnknownPtr<IAudioProcessor>(component);
    require(host.processor, "IAudioProcessor");
    ProcessSetup setup {kRealtime, kSample32, kBlock, kRate};
    require(host.processor->setupProcessing(setup) == kResultOk, "setupProcessing");
    require(component->activateBus(kAudio, kOutput, 0, true) == kResultOk, "activate the audio output");
    require(component->activateBus(kEvent, kInput, 0, true) == kResultOk, "activate the event input");
    require(component->setActive(true) == kResultOk, "setActive");
    require(host.data.prepare(*component, kBlock, kSample32), "process buffers");
    // HostProcessData allocates its buffers uninitialised, and kabl mixes its audio input
    // into the output when the patch does not use `audio.in`. A host feeds that input
    // silence; leftover heap contents would play as noise (or NaN) on top of the oscillator.
    for (int32 bus = 0; bus < host.data.numInputs; ++bus)
        for (int32 channel = 0; channel < host.data.inputs[bus].numChannels; ++channel)
            if (float* samples = host.data.inputs[bus].channelBuffers32[channel])
                std::fill(samples, samples + kBlock, 0.0f);
    host.data.processMode = kRealtime;
    host.context.sampleRate = kRate;
    host.processor->setProcessing(true);

    constexpr int measure = 48; // about half a second
    host.render(40);
    host.note(true);
    host.render(30);
    host.render(measure);
    const double base = frequency(host.last(measure));
    const double level = rms(host.last(measure));
    check(base > 250 && base < 275, fmt("note 60 plays at C4: %.1f Hz, rms %.4f", base, level));

    host.send(bend, 1.0);
    host.render(5);
    host.render(measure);
    const double bent = frequency(host.last(measure));
    check(bent / base > 1.10 && bent / base < 1.15,
          fmt("full pitch bend raises it two semitones: %.1f Hz (x%.4f)", bent, bent / base));
    host.send(bend, 8192.0 / 16383.0);
    host.render(5 + measure);
    const double centred = frequency(host.last(measure));
    check(std::fabs(centred / base - 1) < 0.01, fmt("centred bend returns it: %.1f Hz", centred));

    host.send(wheel, 1.0);
    host.render(5 + measure);
    const double wheeled = frequency(host.last(measure));
    check(wheeled / base > 1.04 && wheeled / base < 1.08,
          fmt("mod wheel (CC 1) at full raises it a semitone: %.1f Hz (x%.4f)", wheeled, wheeled / base));
    host.send(wheel, 0.0);
    host.render(5 + measure);
    const double unwheeled = frequency(host.last(measure));
    check(std::fabs(unwheeled / base - 1) < 0.01, fmt("mod wheel at zero returns it: %.1f Hz", unwheeled));

    // CC 74 drives base_hz through soft takeover: 20 sits below the knob, 64 crosses it.
    host.send(cc74, 20.5 / 127.0);
    host.render(20);
    host.send(cc74, 64.5 / 127.0);
    host.render(100 + measure);
    const double moved = frequency(host.last(measure));
    check(moved / base > 1.5, fmt("CC 74 moves the mapped base frequency: %.1f Hz (x%.4f)", moved, moved / base));

    host.send(sustain, 1.0);
    host.render(2);
    host.note(false);
    host.render(100);
    const double held = rms(host.last(20));
    check(held > 0.25 * level, fmt("sustain (CC 64) holds the released note: rms %.4f", held));
    host.send(sustain, 0.0);
    host.render(200);
    const double released = rms(host.last(20));
    check(released < 0.01 * level, fmt("pedal up releases it: rms %.6f", released));
    const auto broken = std::count_if(host.left.begin(), host.left.end(), [](float v) { return !std::isfinite(v); });
    check(broken == 0, fmt("every output sample is finite: %.0f are not", static_cast<double>(broken)));

    host.processor->setProcessing(false);
    component->setActive(false);
    host.data.unprepare();
    host.processor = nullptr;
    host.component = nullptr;
    component = nullptr;
    controller = nullptr;
    provider = nullptr;
    PluginContextFactory::instance().setPluginContext(nullptr);

    std::printf("%d check(s) failed\n", failures);
    return failures;
}
