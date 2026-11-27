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
// The editor checks (Windows only) open and close kabl's editor 20 times in a parent window, as
// Live does, and check its size against the window's DPI, that a host resize is held to the
// editor's fixed size, and that GDI and USER handles do not grow. On a CI runner they then
// change the display's scale and check that the editor asks the host (IPlugFrame::resizeView)
// to fit its frame, once per change, and does not repeat a refused request. They need an
// OpenGL 3.2 context for the editor's renderer; without one they stop before attached() and
// say so.
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
#include "pluginterfaces/gui/iplugview.h"
#include "pluginterfaces/gui/iplugviewcontentscalesupport.h"

#if SMTG_OS_WINDOWS
#ifndef NOMINMAX
#define NOMINMAX
#endif
#include <windows.h>
#include <GL/gl.h>
#endif

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
    long broken = 0; // non-finite samples on any output bus and channel

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
            for (int32 bus = 0; bus < data.numOutputs; ++bus)
                for (int32 channel = 0; channel < data.outputs[bus].numChannels; ++channel)
                    if (const float* samples = data.outputs[bus].channelBuffers32[channel])
                        broken += std::count_if(samples, samples + kBlock, [](float v) { return !std::isfinite(v); });
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

#if SMTG_OS_WINDOWS
// What a host's IPlugFrame does with a resize request: resize the parent, then the view. It
// counts the requests and keeps the last one; with `refuse` set it turns them down, as a host
// that cannot resize its frame does.
struct Frame : IPlugFrame
{
    HWND parent = nullptr;
    bool refuse = false;
    int requests = 0;
    ViewRect last {};

    tresult PLUGIN_API resizeView(IPlugView* view, ViewRect* rect) override
    {
        ++requests;
        last = *rect;
        if (refuse)
            return kResultFalse;
        SetWindowPos(parent, nullptr, 0, 0, rect->getWidth(), rect->getHeight(), SWP_NOMOVE | SWP_NOZORDER);
        return view->onSize(rect) == kResultOk ? kResultOk : kResultFalse;
    }
    tresult PLUGIN_API queryInterface(const TUID iid, void** obj) override
    {
        if (FUnknownPrivate::iidEqual(iid, IPlugFrame::iid) || FUnknownPrivate::iidEqual(iid, FUnknown::iid))
        {
            *obj = this;
            return kResultOk;
        }
        *obj = nullptr;
        return kNoInterface;
    }
    // Lives on the stack for the whole check.
    uint32 PLUGIN_API addRef() override { return 1; }
    uint32 PLUGIN_API release() override { return 1; }
};

// Whether this machine can make the OpenGL 3.2 core context kabl's editor asks for, with the
// driver's version and renderer for the log.
bool probeGl(std::string& driver)
{
    constexpr int kMajor = 0x2091, kMinor = 0x2092, kProfile = 0x9126, kCore = 0x1;
    using CreateContextAttribs = HGLRC(WINAPI*)(HDC, HGLRC, const int*);
    WNDCLASSW wc {};
    wc.style = CS_OWNDC;
    wc.lpfnWndProc = DefWindowProcW;
    wc.hInstance = GetModuleHandleW(nullptr);
    wc.lpszClassName = L"kabl-gl-probe";
    RegisterClassW(&wc);
    HWND window = CreateWindowExW(0, wc.lpszClassName, L"", WS_OVERLAPPEDWINDOW, 0, 0, 16, 16, nullptr, nullptr,
                                  wc.hInstance, nullptr);
    HDC dc = GetDC(window);
    PIXELFORMATDESCRIPTOR pfd {};
    pfd.nSize = sizeof(pfd);
    pfd.nVersion = 1;
    pfd.dwFlags = PFD_DRAW_TO_WINDOW | PFD_SUPPORT_OPENGL | PFD_DOUBLEBUFFER;
    pfd.iPixelType = PFD_TYPE_RGBA;
    pfd.cColorBits = 32;
    pfd.cDepthBits = 24;
    pfd.cStencilBits = 8;
    SetPixelFormat(dc, ChoosePixelFormat(dc, &pfd), &pfd);
    bool core = false;
    driver = "none (wglCreateContext failed)";
    if (HGLRC legacy = wglCreateContext(dc))
    {
        wglMakeCurrent(dc, legacy);
        const auto* version = reinterpret_cast<const char*>(glGetString(GL_VERSION));
        const auto* renderer = reinterpret_cast<const char*>(glGetString(GL_RENDERER));
        driver = std::string(version ? version : "?") + " on " + (renderer ? renderer : "?");
        auto create = reinterpret_cast<CreateContextAttribs>(wglGetProcAddress("wglCreateContextAttribsARB"));
        const int attributes[] = {kMajor, 3, kMinor, 2, kProfile, kCore, 0};
        if (HGLRC context = create ? create(dc, nullptr, attributes) : nullptr)
        {
            core = true;
            wglDeleteContext(context);
        }
        wglMakeCurrent(nullptr, nullptr);
        wglDeleteContext(legacy);
    }
    ReleaseDC(window, dc);
    DestroyWindow(window);
    return core;
}

void pump(int ms)
{
    const auto end = std::chrono::steady_clock::now() + std::chrono::milliseconds(ms);
    while (std::chrono::steady_clock::now() < end)
    {
        MSG msg;
        while (PeekMessageW(&msg, nullptr, 0, 0, PM_REMOVE))
        {
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
        Sleep(5);
    }
}

SIZE clientSize(HWND window)
{
    RECT r {};
    if (window)
        GetClientRect(window, &r);
    return {r.right - r.left, r.bottom - r.top};
}

std::string sizes(const char* what, double width, double height)
{
    return fmt((std::string(what) + " %.0f x %.0f").c_str(), width, height);
}

// The primary display's scale, as steps relative to the one Windows recommends for it (0), through
// the undocumented DisplayConfig calls the Settings app uses: they apply at once and send
// per-monitor-aware windows WM_DPICHANGED, as a move to another monitor does.
struct DisplayScale
{
    DISPLAYCONFIG_PATH_INFO path {};
    bool found = false;
    int32_t min = 0, current = 0, max = 0;

    bool read()
    {
        UINT32 paths = 0, modes = 0;
        if (GetDisplayConfigBufferSizes(QDC_ONLY_ACTIVE_PATHS, &paths, &modes) != ERROR_SUCCESS || paths == 0)
            return found = false;
        std::vector<DISPLAYCONFIG_PATH_INFO> pathInfo(paths);
        std::vector<DISPLAYCONFIG_MODE_INFO> modeInfo(modes);
        if (QueryDisplayConfig(QDC_ONLY_ACTIVE_PATHS, &paths, pathInfo.data(), &modes, modeInfo.data(), nullptr) !=
            ERROR_SUCCESS)
            return found = false;
        path = pathInfo[0];
        struct
        {
            DISPLAYCONFIG_DEVICE_INFO_HEADER header;
            int32_t min, current, max;
        } get {};
        get.header.type = static_cast<DISPLAYCONFIG_DEVICE_INFO_TYPE>(-3);
        get.header.size = sizeof get;
        get.header.adapterId = path.sourceInfo.adapterId;
        get.header.id = path.sourceInfo.id;
        if (DisplayConfigGetDeviceInfo(&get.header) != ERROR_SUCCESS)
            return found = false;
        min = get.min;
        current = get.current;
        max = get.max;
        return found = true;
    }

    bool set(int32_t step)
    {
        struct
        {
            DISPLAYCONFIG_DEVICE_INFO_HEADER header;
            int32_t step;
        } put {};
        put.header.type = static_cast<DISPLAYCONFIG_DEVICE_INFO_TYPE>(-4);
        put.header.size = sizeof put;
        put.header.adapterId = path.sourceInfo.adapterId;
        put.header.id = path.sourceInfo.id;
        put.step = step;
        return DisplayConfigSetDeviceInfo(&put.header) == ERROR_SUCCESS;
    }
};

// Pumps messages until the window's DPI is no longer `from`, for up to five seconds.
UINT waitForDpiChange(HWND window, UINT from)
{
    for (int i = 0; i < 100 && GetDpiForWindow(window) == from; ++i)
        pump(50);
    return GetDpiForWindow(window);
}

// A DPI change with the editor open (the host window moving to another monitor) and a first open
// at a scale other than 100 % must each ask the host, once, to fit its frame to the editor's new
// size; a host that refuses is not asked again and the editor keeps its size. Changes the
// display's scale, so it runs on CI runners only, and puts the scale back.
void dpiChecks(IEditController* controller, int32 kWidth, int32 kHeight)
{
    if (!std::getenv("GITHUB_ACTIONS"))
    {
        std::printf("editor: SKIPPED the DPI change checks, which change the display's scale (CI only)\n");
        return;
    }
    DisplayScale display;
    bool raisedResolution = false;
    if (display.read() && display.max <= display.current)
    {
        // Windows offers no larger scale on a small display; take its largest mode.
        DEVMODEW best {}, mode {};
        best.dmSize = mode.dmSize = sizeof(DEVMODEW);
        for (DWORD i = 0; EnumDisplaySettingsW(nullptr, i, &mode); ++i)
            if (mode.dmPelsWidth * mode.dmPelsHeight > best.dmPelsWidth * best.dmPelsHeight)
                best = mode;
        best.dmFields = DM_PELSWIDTH | DM_PELSHEIGHT;
        raisedResolution = best.dmPelsWidth && ChangeDisplaySettingsW(&best, 0) == DISP_CHANGE_SUCCESSFUL;
        std::printf("editor: display resolution raised to %lu x %lu: %s\n", best.dmPelsWidth, best.dmPelsHeight,
                    raisedResolution ? "yes" : "no");
        pump(500);
        display.read();
    }
    std::printf("editor: display scale steps %s: min %d, current %d, max %d\n", display.found ? "read" : "unreadable",
                display.min, display.current, display.max);
    const int32_t original = display.current;
    const int32_t raised = std::min(display.max, original + 2);
    auto restore = [&] {
        display.set(original);
        if (raisedResolution)
            ChangeDisplaySettingsW(nullptr, 0);
        pump(500);
    };

    WNDCLASSW wc {};
    wc.lpfnWndProc = DefWindowProcW;
    wc.hInstance = GetModuleHandleW(nullptr);
    wc.lpszClassName = L"kabl-vst3-host-dpi";
    RegisterClassW(&wc);
    HWND parent = CreateWindowExW(0, wc.lpszClassName, L"kabl-vst3-host", WS_POPUP | WS_VISIBLE, 0, 0, kWidth,
                                  kHeight, nullptr, nullptr, wc.hInstance, nullptr);
    require(parent != nullptr, "create the parent window");
    Frame frame;
    frame.parent = parent;
    auto child = [&] { return GetWindow(parent, GW_CHILD); };

    // Opens the editor in `parent` at the parent's current scale, as a host does.
    auto open = [&]() -> IPtr<IPlugView> {
        IPtr<IPlugView> view = owned(controller->createView(ViewType::kEditor));
        if (!view)
            return view;
        view->setFrame(&frame);
        const double scale = GetDpiForWindow(parent) / 96.0;
        if (FUnknownPtr<IPlugViewContentScaleSupport> scaling(view); scaling)
            scaling->setContentScaleFactor(static_cast<float>(scale));
        ViewRect rect {};
        view->getSize(&rect);
        SetWindowPos(parent, nullptr, 0, 0, rect.getWidth(), rect.getHeight(), SWP_NOMOVE | SWP_NOZORDER);
        if (view->attached(parent, kPlatformTypeHWND) != kResultOk)
        {
            view->setFrame(nullptr);
            return nullptr;
        }
        pump(300);
        return view;
    };
    auto close = [&](IPtr<IPlugView>& view) {
        if (view)
        {
            view->removed();
            view->setFrame(nullptr);
        }
        view = nullptr;
        pump(100);
    };
    auto report = [&](IPlugView* view) {
        ViewRect now {};
        if (view)
            view->getSize(&now);
        const SIZE window = clientSize(child());
        char buffer[200];
        std::snprintf(buffer, sizeof buffer, "%d request(s), last %d x %d; getSize %d x %d; window %ld x %ld",
                      frame.requests, frame.last.getWidth(), frame.last.getHeight(), now.getWidth(), now.getHeight(),
                      window.cx, window.cy);
        return std::string(buffer);
    };
    // One request, and it equals getSize, the editor window and the editor's size at `dpi`.
    auto askedOnce = [&](IPlugView* view, UINT dpi) {
        ViewRect now {};
        view->getSize(&now);
        const SIZE window = clientSize(child());
        const LONG width = std::lround(kWidth * dpi / 96.0), height = std::lround(kHeight * dpi / 96.0);
        return frame.requests == 1 && frame.last.getWidth() == width && frame.last.getHeight() == height &&
               now.getWidth() == width && now.getHeight() == height && window.cx == width && window.cy == height;
    };

    // The editor is open when the scale goes up.
    IPtr<IPlugView> view = open();
    check(view != nullptr, "the editor opens for the DPI change checks");
    if (!view)
    {
        DestroyWindow(parent);
        restore();
        return;
    }
    const UINT before = GetDpiForWindow(child());
    frame.requests = 0;
    const bool set = display.found && raised > original && display.set(raised);
    const UINT after = waitForDpiChange(child(), before);
    std::printf("editor: display scale step %d -> %d: %s; editor window %u -> %u DPI\n", original, raised,
                set ? "set" : "not set", before, after);
    check(after != before, "the display scale change reaches the editor window's DPI");
    if (after == before)
    {
        close(view);
        DestroyWindow(parent);
        restore();
        return;
    }
    pump(500);
    check(askedOnce(view, after), "a DPI change with the editor open asks the host once to fit its frame to getSize "
                                  "and the editor window: " +
                                      report(view));

    // A first open at the raised scale.
    close(view);
    frame.requests = 0;
    view = open();
    pump(300);
    check(view && askedOnce(view, after),
          "opening the editor at the raised scale asks the host once to fit its frame: " + report(view));

    // The host refuses, and the scale goes back down.
    if (view)
    {
        frame.refuse = true;
        frame.requests = 0;
        display.set(original);
        const UINT back = waitForDpiChange(child(), after);
        pump(1000);
        const SIZE kept = clientSize(child());
        const LONG width = std::lround(kWidth * back / 96.0), height = std::lround(kHeight * back / 96.0);
        check(back != after && frame.requests == 1 && kept.cx == width && kept.cy == height,
              fmt("a refused request is not repeated and the editor keeps its rescaled size at %.0f DPI: ", back) +
                  report(view));
    }

    close(view);
    DestroyWindow(parent);
    restore();
}

void editorChecks(IEditController* controller)
{
    constexpr int kCycles = 20;
    // A leak of one handle per cycle grows the count by 19 from cycle 1 to cycle 20.
    constexpr DWORD kTolerance = 10;
    constexpr int32 kWidth = 1180, kHeight = 680; // EguiState::from_size in crates/clap/src/lib.rs

    std::string driver;
    const bool gl = probeGl(driver);
    std::printf("editor: OpenGL %s; 3.2 core context: %s\n", driver.c_str(), gl ? "yes" : "no");
    if (!gl)
        std::printf("editor: SKIPPED attached()/removed(), which open the editor window: kabl renders it with "
                    "OpenGL 3.2, which this machine cannot create. Every cycle still runs createView, "
                    "setContentScaleFactor, getSize, the resize checks and the release.\n");

    WNDCLASSW wc {};
    wc.lpfnWndProc = DefWindowProcW;
    wc.hInstance = GetModuleHandleW(nullptr);
    wc.lpszClassName = L"kabl-vst3-host";
    RegisterClassW(&wc);
    HWND parent = CreateWindowExW(0, wc.lpszClassName, L"kabl-vst3-host", WS_POPUP | WS_VISIBLE, 0, 0, kWidth,
                                  kHeight, nullptr, nullptr, wc.hInstance, nullptr);
    require(parent != nullptr, "create the parent window");
    const UINT dpi = GetDpiForWindow(parent);
    const double scale = dpi / 96.0;
    const LONG scaledWidth = std::lround(kWidth * scale);
    const LONG scaledHeight = std::lround(kHeight * scale);
    std::printf("editor: parent window at %u DPI (scale %.2f)\n", dpi, scale);

    Frame frame;
    frame.parent = parent;
    const HANDLE process = GetCurrentProcess();
    const DWORD gdiStart = GetGuiResources(process, GR_GDIOBJECTS);
    const DWORD userStart = GetGuiResources(process, GR_USEROBJECTS);
    DWORD gdiFirst = 0, userFirst = 0;
    int created = 0, sized = 0, attached = 0, opened = 0, gone = 0;
    std::string firstProblem;
    auto problem = [&](int cycle, const std::string& what) {
        if (firstProblem.empty())
            firstProblem = "cycle " + std::to_string(cycle) + ": " + what;
    };

    for (int cycle = 1; cycle <= kCycles; ++cycle)
    {
        IPtr<IPlugView> view = owned(controller->createView(ViewType::kEditor));
        if (!view || view->isPlatformTypeSupported(kPlatformTypeHWND) != kResultTrue)
        {
            problem(cycle, "createView or HWND support");
            continue;
        }
        ++created;
        view->setFrame(&frame);
        if (FUnknownPtr<IPlugViewContentScaleSupport> scaling(view); scaling)
            scaling->setContentScaleFactor(static_cast<float>(scale));
        ViewRect rect {};
        if (view->getSize(&rect) == kResultOk && rect.getWidth() == kWidth && rect.getHeight() == kHeight)
            ++sized;
        else
            problem(cycle, sizes("getSize before attached", rect.getWidth(), rect.getHeight()));

        if (cycle == 1)
        {
            check(view->canResize() == kResultFalse, "the editor tells the host it is not resizable");
            ViewRect asked {0, 0, kWidth * 3 / 2, kHeight * 3 / 2};
            const bool held = view->checkSizeConstraint(&asked) == kResultOk;
            check(held && asked.getWidth() == kWidth && asked.getHeight() == kHeight,
                  sizes("checkSizeConstraint holds a 1770 x 1020 request to the editor size:", asked.getWidth(),
                        asked.getHeight()));
        }

        HWND child = nullptr;
        bool afterRemoved = false;
        if (gl)
        {
            SetWindowPos(parent, nullptr, 0, 0, rect.getWidth(), rect.getHeight(), SWP_NOMOVE | SWP_NOZORDER);
            if (view->attached(parent, kPlatformTypeHWND) == kResultOk)
            {
                ++attached;
                pump(150);
                child = GetWindow(parent, GW_CHILD);
                ViewRect now {};
                view->getSize(&now);
                const SIZE size = clientSize(child);
                // The editor window follows the DPI, and getSize reports what the window is.
                if (child && size.cx == scaledWidth && size.cy == scaledHeight && now.getWidth() == size.cx &&
                    now.getHeight() == size.cy)
                    ++opened;
                else
                    problem(cycle, sizes("editor window", size.cx, size.cy) + ", " +
                                       sizes("getSize after attached", now.getWidth(), now.getHeight()));

                if (cycle == 1)
                {
                    ViewRect asked {0, 0, kWidth * 3 / 2, kHeight * 3 / 2};
                    view->onSize(&asked);
                    pump(50);
                    ViewRect after {};
                    view->getSize(&after);
                    const SIZE held = clientSize(child);
                    check(after.getWidth() == scaledWidth && after.getHeight() == scaledHeight &&
                              held.cx == scaledWidth && held.cy == scaledHeight,
                          sizes("onSize(1770 x 1020) leaves the open editor at its size: window", held.cx, held.cy) +
                              ", " + sizes("getSize", after.getWidth(), after.getHeight()));
                }

                view->removed();
                pump(20);
                afterRemoved = child && IsWindow(child);
            }
            else
                problem(cycle, "attached");
        }
        view->setFrame(nullptr);
        view = nullptr;
        // clap-wrapper keeps the editor window through removed() and destroys it with the view,
        // which a host releases after removed(); a host's message loop keeps running meanwhile.
        pump(100);
        if (child)
        {
            if (!IsWindow(child))
                ++gone;
            else
                problem(cycle, "the editor window outlived the released view");
        }
        if (gl && (cycle <= 3 || cycle == kCycles))
            std::printf("editor: cycle %d: window alive after removed() %s, after release %s; GDI %lu, USER %lu\n",
                        cycle, afterRemoved ? "yes" : "no", child && IsWindow(child) ? "yes" : "no",
                        GetGuiResources(process, GR_GDIOBJECTS), GetGuiResources(process, GR_USEROBJECTS));
        if (cycle == 1)
        {
            gdiFirst = GetGuiResources(process, GR_GDIOBJECTS);
            userFirst = GetGuiResources(process, GR_USEROBJECTS);
        }
    }
    const DWORD gdiEnd = GetGuiResources(process, GR_GDIOBJECTS);
    const DWORD userEnd = GetGuiResources(process, GR_USEROBJECTS);
    DestroyWindow(parent);

    if (!firstProblem.empty())
        std::printf("editor: first problem: %s\n", firstProblem.c_str());
    check(created == kCycles, fmt("createView gives an HWND editor %.0f of %.0f times", created, kCycles));
    check(sized == kCycles, fmt("getSize is 1180 x 680 before attached %.0f of %.0f times", sized, kCycles));
    if (gl)
    {
        check(attached == kCycles, fmt("attached() opens the editor %.0f of %.0f times", attached, kCycles));
        check(opened == kCycles, fmt("the open editor is 1180 x 680 times the DPI scale and getSize says so "
                                     "%.0f of %.0f times",
                                     opened, kCycles));
        check(gone == kCycles, fmt("releasing the view destroys the editor window %.0f of %.0f times", gone, kCycles));
    }
    std::printf("editor: GDI objects %lu at start, %lu after cycle 1, %lu after cycle %d; "
                "USER objects %lu, %lu, %lu\n",
                gdiStart, gdiFirst, gdiEnd, kCycles, userStart, userFirst, userEnd);
    check(gdiEnd <= gdiFirst + kTolerance,
          fmt("GDI objects grow by at most 10 from cycle 1 to cycle 20: %+.0f", double(gdiEnd) - double(gdiFirst)));
    check(userEnd <= userFirst + kTolerance,
          fmt("USER objects grow by at most 10 from cycle 1 to cycle 20: %+.0f", double(userEnd) - double(userFirst)));
    if (gl)
    {
        // At 100 % the host sized its frame for the editor already; at any other scale each open asks once.
        const int expected = scale == 1.0 ? 0 : attached;
        check(frame.requests == expected,
              fmt("the editor asks the host to resize %.0f times (expected %.0f)", frame.requests, expected));
        dpiChecks(controller, kWidth, kHeight);
    }
}
#endif

} // namespace

int main(int argc, char* argv[])
{
#if SMTG_OS_WINDOWS
    // As DAWs are: windows get the DPI of the monitor they are on and hear when it changes.
    SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
#endif
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
    check(host.broken == 0, fmt("every output sample is finite: %.0f are not", static_cast<double>(host.broken)));

#if SMTG_OS_WINDOWS
    editorChecks(controller);
#else
    std::printf("editor: the editor checks are Windows only\n");
#endif

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
