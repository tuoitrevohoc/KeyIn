#import <Cocoa/Cocoa.h>
#import <InputMethodKit/InputMethodKit.h>
#import <Carbon/Carbon.h>
#import <QuartzCore/QuartzCore.h>
#import "KeyInRime.h"

extern char *keyin_compose_context(const char *, unsigned int, unsigned int);
extern void keyin_free(char *);
extern unsigned int keyin_language_options(const char *);
extern char *keyin_correct(const char *, unsigned int);
static IMKServer *server;
static NSRange currentRange(void) { return NSMakeRange(NSNotFound, NSNotFound); }

@interface KeyInController : IMKInputController
@property NSMutableString *raw;
@property NSString *preedit;
@property NSString *language;
@property NSArray<NSString *> *choices;
@property NSUInteger page;
@property NSPanel *panel;
@property NSTextField *label;
@property NSTextField *status;
@property NSString *correction;
@property NSString *correctionLanguage;
@property KeyInRime *rime;
@property BOOL usingRime;
@property BOOL rimeEditing;
@property NSMutableArray<NSNumber *> *recentLanguages;
@property NSUInteger markedCaret;
@property unsigned int wordMode;
@property NSArray<NSNumber *> *languageOptions;
@property EventHotKeyRef hotkey;
@property EventHandlerRef handler;
- (void)cycle;
@end
static OSStatus switchLanguage(EventHandlerCallRef next, EventRef event, void *context) {
    [(__bridge KeyInController *)context cycle];
    return noErr;
}
@implementation KeyInController
- (id)initWithServer:(IMKServer *)server delegate:(id)delegate client:(id)client {
    self = [super initWithServer:server delegate:delegate client:client];
    if (self) { _raw = [NSMutableString new]; _choices = @[]; _preedit = @""; _rime = [KeyInRime new]; _recentLanguages = [NSMutableArray new]; }
    return self;
}
- (unsigned int)mode {
    NSUserDefaults *d = NSUserDefaults.standardUserDefaults;
    return [d boolForKey:@"AutomaticDetection"] ? 0 : (unsigned int)[d integerForKey:@"Language"];
}
- (void)activateServer:(id)sender {
    self.language = self.mode ? @[@"English", @"English", @"Vietnamese", @"Chinese"][MIN(self.mode, 3)] : (self.language ?: @"English");
    [self showCandidates:sender];
    if (!self.handler) {
        EventTypeSpec type = { kEventClassKeyboard, kEventHotKeyPressed };
        InstallEventHandler(GetApplicationEventTarget(), switchLanguage, 1, &type, (__bridge void *)self, &_handler);
        EventHotKeyID key = { 'KyIn', 1 };
        RegisterEventHotKey(kVK_Space, cmdKey, key, GetApplicationEventTarget(), 0, &_hotkey);
    }
}
- (void)releaseHotkey {
    if (_hotkey) { UnregisterEventHotKey(_hotkey); _hotkey = NULL; }
    if (_handler) { RemoveEventHandler(_handler); _handler = NULL; }
}
- (void)deactivateServer:(id)sender { [self commitComposition:sender]; [self.recentLanguages removeAllObjects]; if (!self.mode) self.language = @"English"; [self.panel orderOut:nil]; [self releaseHotkey]; }
- (void)inputControllerWillClose { self.rime = nil; [self.panel orderOut:nil]; [self releaseHotkey]; }
- (void)hidePalettes { [self.panel orderOut:nil]; }
- (unsigned int)contextLanguage {
    if (self.raw.length && [self.language isEqualToString:@"Chinese"]) return 3;
    // The latest confirmed language wins over older history for ambiguous input.
    return self.recentLanguages.lastObject.unsignedIntValue;
}
- (void)rememberLanguage {
    NSUInteger language = [@[@"English", @"Vietnamese", @"Chinese"] indexOfObject:self.language];
    if (language == NSNotFound) return;
    [self.recentLanguages addObject:@(language + 1)];
    if (self.recentLanguages.count > 3) [self.recentLanguages removeObjectAtIndex:0];
}
- (void)renderRime:(id<IMKTextInput>)client {
    if (!self.raw.length) { self.wordMode = 0; self.rimeEditing = NO; }
    [self updateLanguageOptions];
    NSDictionary *state = [self.rime snapshot];
    self.language = @"Chinese";
    self.preedit = state[@"preedit"] ?: self.raw;
    self.choices = state[@"choices"] ?: @[];
    self.markedCaret = state[@"caret"] ? [state[@"caret"] unsignedIntegerValue] : self.preedit.length;
    self.correction = nil; self.correctionLanguage = nil;
    self.page = MIN(self.page, self.choices.count ? (self.choices.count - 1) / 9 : 0);
    [client setMarkedText:self.preedit selectionRange:NSMakeRange(self.markedCaret, 0) replacementRange:currentRange()];
    [self showCandidates:client];
}
- (void)selectRimeCandidate:(NSUInteger)index client:(id<IMKTextInput>)client {
    NSString *commit = [self.rime selectCandidate:index];
    if (commit.length) {
        [self rememberLanguage];
        [self finish:commit client:client];
    } else {
        self.rimeEditing = YES;
        self.page = 0;
        [self renderRime:client];
    }
}
- (void)updateLanguageOptions {
    unsigned int mask = self.mode == 0 ? keyin_language_options(self.raw.UTF8String) : 0;
    if (self.wordMode && self.raw.length) mask |= 1 << self.wordMode;
    NSMutableArray *options = [NSMutableArray new];
    for (unsigned int language = 1; language <= 3; language++)
        if (mask & (1 << language)) [options addObject:@(language)];
    self.languageOptions = options;
}
- (BOOL)stepLanguage:(NSInteger)direction client:(id<IMKTextInput>)client {
    if (self.mode || !self.raw.length) return NO;
    [self updateLanguageOptions];
    if (self.languageOptions.count < 2) return NO;
    NSUInteger active = [@[@"English", @"Vietnamese", @"Chinese"] indexOfObject:self.language];
    NSUInteger index = [self.languageOptions indexOfObject:@(active + 1)];
    if (index == NSNotFound) index = 0;
    NSInteger count = self.languageOptions.count;
    self.wordMode = self.languageOptions[(index + count + direction) % count].unsignedIntValue;
    self.page = 0; self.rimeEditing = NO; self.usingRime = NO;
    [self.rime clear];
    [self refresh:client];
    return YES;
}
- (void)refresh:(id<IMKTextInput>)client {
    if (!self.raw.length) self.wordMode = 0;
    [self updateLanguageOptions];
    unsigned int effectiveMode = self.wordMode ?: self.mode;
    NSUserDefaults *settings = NSUserDefaults.standardUserDefaults;
    self.rime.learning = [settings objectForKey:@"LearnChinese"] == nil || [settings boolForKey:@"LearnChinese"];
    if (self.rimeEditing && self.usingRime) { [self renderRime:client]; return; }
    if (effectiveMode == 3 && self.rime.available && [self.rime setInput:self.raw]) {
        self.usingRime = YES; [self renderRime:client]; return;
    }
    char *result = keyin_compose_context(self.raw.UTF8String, effectiveMode, [self contextLanguage]);
    if (!result) return;
    NSArray *lines = [[NSString stringWithUTF8String:result] componentsSeparatedByString:@"\n"];
    keyin_free(result);
    self.language = lines[0]; self.preedit = lines[1];
    if ([self.language isEqualToString:@"Chinese"] && self.rime.available && [self.rime setInput:self.raw]) {
        self.usingRime = YES; [self renderRime:client]; return;
    }
    self.usingRime = NO;
    [self.rime clear];
    NSMutableArray<NSString *> *choices = [NSMutableArray new];
    NSMutableSet<NSString *> *seen = [NSMutableSet new];
    for (NSUInteger i = 2; i < lines.count; i++) {
        NSString *candidate = [lines[i] stringByApplyingTransform:@"Traditional-Simplified" reverse:NO] ?: lines[i];
        if (![seen containsObject:candidate]) {
            [choices addObject:candidate];
            [seen addObject:candidate];
        }
    }
    self.choices = choices;
    self.correction = nil;
    self.correctionLanguage = nil;
    if (!self.wordMode && [NSUserDefaults.standardUserDefaults boolForKey:@"Autocorrect"]) {
        char *fix = keyin_correct(self.raw.UTF8String, self.mode);
        if (fix) {
            NSArray *parts = [[NSString stringWithUTF8String:fix] componentsSeparatedByString:@"\n"];
            if (parts.count >= 2) {
                self.correctionLanguage = parts[0];
                NSString *value = parts.count > 2 ? parts[2] : parts[1];
                self.correction = [parts[0] isEqualToString:@"Chinese"] ?
                    ([value stringByApplyingTransform:@"Traditional-Simplified" reverse:NO] ?: value) : value;
            }
            keyin_free(fix);
        }
    }
    self.page = MIN(self.page, self.choices.count ? (self.choices.count - 1) / 9 : 0);
    [client setMarkedText:self.preedit selectionRange:NSMakeRange(self.preedit.length, 0) replacementRange:currentRange()];
    [self showCandidates:client];
}
- (void)showCandidates:(id<IMKTextInput>)client {
    BOOL hasChoices = self.choices.count > 0;
    CGFloat width = hasChoices ? 396 : (self.correction ? 310 : 214);
    if (self.languageOptions.count > 1) width = MAX(width, 310);
    NSUInteger start = self.page * 9;
    NSUInteger count = MIN(9, self.choices.count - MIN(start, self.choices.count));
    CGFloat height = hasChoices ? 32 + ((count + 2) / 3) * 24 : 30;
    if (!self.panel) {
        self.panel = [[NSPanel alloc] initWithContentRect:NSMakeRect(0, 0, width, height) styleMask:NSWindowStyleMaskBorderless | NSWindowStyleMaskNonactivatingPanel backing:NSBackingStoreBuffered defer:NO];
        self.panel.level = NSPopUpMenuWindowLevel;
        self.panel.hasShadow = YES;
        self.panel.opaque = YES;
        self.panel.backgroundColor = NSColor.whiteColor;
        self.panel.appearance = [NSAppearance appearanceNamed:NSAppearanceNameAqua];
        self.panel.ignoresMouseEvents = YES;
        NSView *surface = [[NSView alloc] initWithFrame:NSMakeRect(0, 0, width, height)];
        surface.wantsLayer = YES;
        surface.layer.cornerRadius = 0;
        surface.layer.backgroundColor = NSColor.whiteColor.CGColor;
        surface.layer.borderColor = [NSColor colorWithWhite:0.90 alpha:1].CGColor;
        surface.layer.borderWidth = 0.5;
        surface.layer.masksToBounds = YES;
        self.panel.contentView = surface;
        self.status = [NSTextField labelWithString:@""];
        self.status.font = [NSFont systemFontOfSize:12 weight:NSFontWeightSemibold];
        [surface addSubview:self.status];
    }
    [self.panel setContentSize:NSMakeSize(width, height)];
    for (NSView *view in self.panel.contentView.subviews.copy)
        if (view != self.status) [view removeFromSuperview];
    NSString *language = self.correctionLanguage ?: self.language ?: @"English";
    BOOL vi = [language isEqualToString:@"Vietnamese"];
    BOOL zh = [language isEqualToString:@"Chinese"];
    NSString *badge = vi ? @"🇻🇳 VI" : (zh ? @"🇨🇳 简中" : @"🇺🇸 EN");
    self.status.textColor = vi ? NSColor.systemTealColor : (zh ? NSColor.systemOrangeColor : NSColor.systemBlueColor);
    NSString *detail = self.correction ? [NSString stringWithFormat:@"Space → %@ · Esc giữ gốc", self.correction] :
        (hasChoices ? [NSString stringWithFormat:@"%lu/%lu · 1–9 · ← → · Space", self.page + 1, (self.choices.count + 8) / 9] : @"⌘Space");
    self.status.stringValue = [NSString stringWithFormat:@"%@  ·  %@  ·  %@", badge, self.wordMode ? @"Chọn" : (self.mode ? @"Manual" : @"Auto"), detail];
    if (self.languageOptions.count > 1) {
        NSMutableArray *flags = [NSMutableArray new];
        NSArray *labels = @[@"", @"🇺🇸", @"🇻🇳", @"🇨🇳"];
        for (NSNumber *option in self.languageOptions) [flags addObject:labels[option.unsignedIntegerValue]];
        self.status.stringValue = [self.status.stringValue stringByAppendingFormat:@" · ↑↓ %@", [flags componentsJoinedByString:@" "]];
    }
    self.status.frame = NSMakeRect(6, height - 24, width - 12, 18);
    self.status.lineBreakMode = NSLineBreakByTruncatingTail;
    self.panel.accessibilityLabel = self.status.stringValue;
    for (NSUInteger i = 0; i < count; i++) {
        NSTextField *item = [NSTextField labelWithString:[NSString stringWithFormat:@"%lu  %@", i + 1, self.choices[start + i]]];
        item.font = [NSFont systemFontOfSize:15 weight:i == 0 ? NSFontWeightSemibold : NSFontWeightRegular];
        item.textColor = i == 0 ? NSColor.controlAccentColor : NSColor.labelColor;
        item.lineBreakMode = NSLineBreakByTruncatingTail;
        item.toolTip = self.choices[start + i];
        item.frame = NSMakeRect(6 + (i % 3) * 128, height - 50 - (i / 3) * 24, 124, 22);
        [self.panel.contentView addSubview:item];
    }
    NSRect caret = NSZeroRect;
    [client attributesForCharacterIndex:0 lineHeightRectangle:&caret];
    NSScreen *screen = NSScreen.mainScreen;
    for (NSScreen *s in NSScreen.screens) if (NSPointInRect(caret.origin, s.frame)) { screen = s; break; }
    NSRect bounds = screen.visibleFrame;
    CGFloat x = MAX(NSMinX(bounds), MIN(caret.origin.x, NSMaxX(bounds) - width));
    CGFloat y = caret.origin.y - height - 4;
    if (y < NSMinY(bounds)) y = NSMaxY(caret) + 4;
    y = MAX(NSMinY(bounds), MIN(y, NSMaxY(bounds) - height));
    [self.panel setFrameOrigin:NSMakePoint(x, y)];
    [self.panel orderFront:nil];
}
- (void)finish:(NSString *)text client:(id<IMKTextInput>)client {
    if (self.raw.length) [client insertText:text replacementRange:currentRange()];
    [self.rime clear]; self.usingRime = NO; self.rimeEditing = NO;
    self.wordMode = 0; self.languageOptions = @[];
    [self.raw setString:@""]; self.preedit = @""; self.choices = @[]; self.page = 0;
    self.correction = nil; self.correctionLanguage = nil;
    [self showCandidates:client];
}
- (void)commitComposition:(id)sender {
    // Focus changes preserve unselected Pinyin instead of silently choosing a character.
    [self finish:[self.language isEqualToString:@"Chinese"] ? self.raw.copy : (self.preedit ?: self.raw) client:sender ?: self.client];
}
- (BOOL)handleEvent:(NSEvent *)event client:(id<IMKTextInput>)client {
    if (event.type != NSEventTypeKeyDown) return NO;
    NSEventModifierFlags flags = event.modifierFlags & NSEventModifierFlagDeviceIndependentFlagsMask;
    if (event.keyCode == kVK_Space && (flags & (NSEventModifierFlagCommand | NSEventModifierFlagOption | NSEventModifierFlagControl | NSEventModifierFlagShift)) == NSEventModifierFlagCommand) { [self cycle]; return YES; }
    if (flags & (NSEventModifierFlagCommand | NSEventModifierFlagControl | NSEventModifierFlagOption)) { [self commitComposition:client]; return NO; }
    if (!(flags & NSEventModifierFlagShift) && (event.keyCode == kVK_UpArrow || event.keyCode == kVK_DownArrow)) {
        if ([self stepLanguage:event.keyCode == kVK_DownArrow ? 1 : -1 client:client]) return YES;
        [self commitComposition:client]; return NO;
    }
    if (self.usingRime && self.raw.length &&
        (event.keyCode == kVK_Home || event.keyCode == kVK_End ||
        ((flags & NSEventModifierFlagShift) && (event.keyCode == kVK_LeftArrow || event.keyCode == kVK_RightArrow)))) {
        int key = event.keyCode == kVK_Home ? 0xff50 : (event.keyCode == kVK_End ? 0xff57 : (event.keyCode == kVK_LeftArrow ? 0xff51 : 0xff53));
        [self.rime processKey:key]; self.rimeEditing = YES; self.page = 0;
        [self renderRime:client]; return YES;
    }
    if (event.keyCode == kVK_Delete && self.raw.length) {
        if (self.usingRime) {
            NSString *before = self.raw.copy;
            NSUInteger caret = self.rime.caret;
            [self.rime processKey:0xff08];
            NSString *updated = self.rime.input;
            if (![before.lowercaseString isEqualToString:updated]) {
                if (caret > 0 && caret <= before.length) {
                    NSMutableString *edited = before.mutableCopy;
                    [edited deleteCharactersInRange:NSMakeRange(caret - 1, 1)];
                    [self.raw setString:[edited.lowercaseString isEqualToString:updated] ? edited : updated];
                } else [self.raw setString:updated];
            }
            self.page = 0;
            [self renderRime:client]; return YES;
        }
        [self.raw deleteCharactersInRange:NSMakeRange(self.raw.length - 1, 1)]; self.page = 0;
        [self refresh:client]; return YES;
    }
    if (event.keyCode == kVK_Escape && self.raw.length) { [self finish:self.raw.copy client:client]; return YES; }
    if (self.choices.count && (event.keyCode == kVK_LeftArrow || event.keyCode == kVK_RightArrow)) {
        NSUInteger pages = (self.choices.count + 8) / 9;
        self.page = (self.page + (event.keyCode == kVK_RightArrow ? 1 : pages - 1)) % pages;
        [self showCandidates:client]; return YES;
    }
    NSString *text = event.characters;
    if (!text.length) return NO;
    unichar c = [text characterAtIndex:0];
    if (self.choices.count && c >= '1' && c <= '9') {
        NSUInteger index = self.page * 9 + c - '1';
        if (index < self.choices.count) {
            if (self.usingRime) [self selectRimeCandidate:index client:client];
            else { [self rememberLanguage]; [self finish:self.choices[index] client:client]; }
        }
        return YES;
    }
    if (event.keyCode == kVK_Space || event.keyCode == kVK_Return || event.keyCode == kVK_Tab) {
        if (!self.raw.length) return NO;
        if (self.usingRime && self.choices.count) {
            [self selectRimeCandidate:self.page * 9 client:client];
            // Keep focus until a partially selected phrase is fully committed.
            return self.raw.length || event.keyCode != kVK_Tab;
        }
        BOOL chinese = self.choices.count > 0 || [self.correctionLanguage isEqualToString:@"Chinese"];
        NSString *value = self.correction ?: (chinese ? self.choices[self.page * 9] : ([self.language isEqualToString:@"Chinese"] ? self.raw.copy : self.preedit));
        if (self.correctionLanguage) self.language = self.correctionLanguage;
        [self rememberLanguage];
        [self finish:value client:client];
        return chinese && event.keyCode != kVK_Tab;
    }
    if (text.length == 1 && ((c >= 'a' && c <= 'z') || (c >= 'A' && c <= 'Z') || (c == '\'' && self.raw.length))) {
        if (self.raw.length >= 128) [self commitComposition:client];
        if (self.usingRime && self.rimeEditing) {
            NSMutableString *edited = self.raw.mutableCopy;
            [edited insertString:text atIndex:MIN(self.rime.caret, edited.length)];
            if ([self.rime processKey:(int)tolower(c)]) {
                NSString *updated = self.rime.input;
                [self.raw setString:[edited.lowercaseString isEqualToString:updated] ? edited : updated]; self.page = 0;
                [self renderRime:client]; return YES;
            }
        }
        [self.raw appendString:text]; self.page = 0; [self refresh:client]; return YES;
    }
    [self commitComposition:client]; return NO;
}
- (void)cycle {
    [self commitComposition:self.client];
    NSUserDefaults *d = NSUserDefaults.standardUserDefaults;
    NSInteger old = [d boolForKey:@"AutomaticDetection"] ? ([@[@"English", @"Vietnamese", @"Chinese"] indexOfObject:self.language ?: @"English"] + 1) : [d integerForKey:@"Language"];
    [d setInteger:old % 3 + 1 forKey:@"Language"];
    [d setBool:NO forKey:@"AutomaticDetection"];
    [self.recentLanguages removeAllObjects];
    self.language = @[@"English", @"English", @"Vietnamese", @"Chinese"][self.mode];
    [self showCandidates:self.client];
}
- (void)toggleAutomatic:(id)sender {
    [self commitComposition:self.client];
    NSUserDefaults *d = NSUserDefaults.standardUserDefaults;
    [d setBool:![d boolForKey:@"AutomaticDetection"] forKey:@"AutomaticDetection"];
    [self.recentLanguages removeAllObjects];
    [self showCandidates:self.client];
}
- (void)selectLanguage:(NSDictionary *)sender {
    [self commitComposition:self.client];
    NSMenuItem *item = sender[(NSString *)kIMKCommandMenuItemName];
    [NSUserDefaults.standardUserDefaults setInteger:item.tag forKey:@"Language"];
    [NSUserDefaults.standardUserDefaults setBool:NO forKey:@"AutomaticDetection"];
    [self.recentLanguages removeAllObjects];
    self.language = @[@"English", @"English", @"Vietnamese", @"Chinese"][self.mode];
    [self showCandidates:self.client];
}
- (void)toggleChineseLearning:(id)sender {
    [self commitComposition:self.client];
    NSUserDefaults *d = NSUserDefaults.standardUserDefaults;
    [d setBool:![d boolForKey:@"LearnChinese"] forKey:@"LearnChinese"];
    [self refresh:self.client];
}
- (void)toggleAutocorrect:(id)sender {
    NSUserDefaults *d = NSUserDefaults.standardUserDefaults;
    [d setBool:![d boolForKey:@"Autocorrect"] forKey:@"Autocorrect"];
    [self refresh:self.client];
}
- (NSMenu *)menu {
    NSMenu *menu = [NSMenu new];
    NSMenuItem *automatic = [menu addItemWithTitle:@"Automatically detect language" action:@selector(toggleAutomatic:) keyEquivalent:@""];
    automatic.state = self.mode == 0 ? NSControlStateValueOn : NSControlStateValueOff;
    NSMenuItem *correct = [menu addItemWithTitle:@"Automatically correct typos" action:@selector(toggleAutocorrect:) keyEquivalent:@""];
    correct.state = [NSUserDefaults.standardUserDefaults boolForKey:@"Autocorrect"] ? NSControlStateValueOn : NSControlStateValueOff;
    NSMenuItem *learn = [menu addItemWithTitle:@"Learn Chinese candidate choices (local)" action:@selector(toggleChineseLearning:) keyEquivalent:@""];
    learn.state = [NSUserDefaults.standardUserDefaults boolForKey:@"LearnChinese"] ? NSControlStateValueOn : NSControlStateValueOff;
    [menu addItem:NSMenuItem.separatorItem];
    NSArray *names = @[@"English", @"Vietnamese — Telex", @"Chinese — Pinyin (Simplified)"];
    for (NSUInteger i = 0; i < names.count; i++) {
        NSMenuItem *item = [menu addItemWithTitle:names[i] action:@selector(selectLanguage:) keyEquivalent:@""];
        item.tag = i + 1; item.state = self.mode == i + 1 ? NSControlStateValueOn : NSControlStateValueOff;
    }
    [menu addItem:NSMenuItem.separatorItem];
    NSMenuItem *engine = [menu addItemWithTitle:self.rime.available ? [@"Chinese engine: Rime " stringByAppendingString:KeyInRime.version] : @"Chinese engine: fallback" action:nil keyEquivalent:@""];
    engine.enabled = NO;
    NSMenuItem *hint = [menu addItemWithTitle:@"⌘Space: cycle languages (manual mode)" action:nil keyEquivalent:@""];
    hint.enabled = NO;
    return menu;
}
@end
int main(void) {
    @autoreleasepool {
        [NSApplication sharedApplication];
        [NSUserDefaults.standardUserDefaults registerDefaults:@{@"AutomaticDetection": @YES, @"Autocorrect": @YES, @"LearnChinese": @YES, @"Language": @1}];
        NSString *shared = [NSBundle.mainBundle.resourcePath stringByAppendingPathComponent:@"rime"];
        NSString *user = [NSHomeDirectory() stringByAppendingPathComponent:@"Library/Application Support/KeyIn/Rime"];
        [KeyInRime startWithSharedData:shared userData:user];
        server = [[IMKServer alloc] initWithName:@"KeyIn_Connection" bundleIdentifier:NSBundle.mainBundle.bundleIdentifier];
        if (!server) return 1;
        [NSApp run];
    }
    return 0;
}
