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
// Languages are bits 1–3 (EN, VI, 中); at least one always stays enabled.
static unsigned int enabledLanguages(void) {
    unsigned int mask = (unsigned int)[NSUserDefaults.standardUserDefaults integerForKey:@"EnabledLanguages"] & 14;
    return mask ?: 14;
}
static unsigned int firstLanguage(unsigned int mask) {
    for (unsigned int language = 1; language <= 3; language++) if (mask & (1 << language)) return language;
    return 1;
}
static BOOL tabShortcuts(void) {
    NSUserDefaults *d = NSUserDefaults.standardUserDefaults;
    return [d objectForKey:@"TabShortcuts"] == nil || [d boolForKey:@"TabShortcuts"];
}

@interface KeyInSettings : NSObject <NSWindowDelegate>
@property NSPanel *window;
@property NSSegmentedControl *languages;
@property NSButton *automatic, *autocorrect, *tabs;
@property NSPopUpButton *meanings;
+ (instancetype)shared;
- (void)show;
@end
@implementation KeyInSettings
+ (instancetype)shared {
    static KeyInSettings *settings;
    static dispatch_once_t once;
    dispatch_once(&once, ^{ settings = [KeyInSettings new]; });
    return settings;
}
- (NSButton *)checkbox:(NSString *)title action:(SEL)action {
    return [NSButton checkboxWithTitle:title target:self action:action];
}
- (void)build {
    self.window = [[NSPanel alloc] initWithContentRect:NSMakeRect(0, 0, 360, 200)
        styleMask:NSWindowStyleMaskTitled | NSWindowStyleMaskClosable | NSWindowStyleMaskNonactivatingPanel
        backing:NSBackingStoreBuffered defer:NO];
    // The input method is a background-only app, so the panel must take focus without activating it.
    self.window.title = @"KeyIn";
    self.window.level = NSFloatingWindowLevel;
    self.window.hidesOnDeactivate = NO;
    self.window.becomesKeyOnlyIfNeeded = NO;
    self.window.releasedWhenClosed = NO;
    self.window.delegate = self;

    self.languages = [NSSegmentedControl segmentedControlWithLabels:@[@"English", @"Tiếng Việt", @"中文"]
        trackingMode:NSSegmentSwitchTrackingSelectAny target:self action:@selector(toggleLanguage:)];
    self.automatic = [self checkbox:@"Tự động nhận diện" action:@selector(toggleAutomatic:)];
    self.autocorrect = [self checkbox:@"Tự sửa lỗi gõ" action:@selector(toggleAutocorrect:)];
    self.tabs = [self checkbox:@"Tab đổi tiếng, ⇧Tab bật/tắt tự động" action:@selector(toggleTabs:)];
    self.meanings = [NSPopUpButton new];
    [self.meanings addItemsWithTitles:@[@"Tắt", @"Tiếng Việt", @"English"]];
    self.meanings.target = self; self.meanings.action = @selector(selectMeanings:);

    NSTextField *(^label)(NSString *) = ^(NSString *text) { return [NSTextField labelWithString:text]; };
    NSView *empty = NSGridCell.emptyContentView;
    NSGridView *grid = [NSGridView gridViewWithViews:@[
        @[label(@"Ngôn ngữ:"), self.languages],
        @[label(@"Gõ:"), self.automatic],
        @[empty, self.autocorrect],
        @[empty, self.tabs],
        @[label(@"Nghĩa chữ Hán:"), self.meanings],
    ]];
    grid.rowSpacing = 10;
    grid.columnSpacing = 8;
    grid.rowAlignment = NSGridRowAlignmentFirstBaseline;
    [grid columnAtIndex:0].xPlacement = NSGridCellPlacementTrailing;
    [grid rowAtIndex:1].topPadding = 6;
    [grid rowAtIndex:4].topPadding = 6;
    grid.translatesAutoresizingMaskIntoConstraints = NO;
    NSView *content = [NSView new];
    [content addSubview:grid];
    [NSLayoutConstraint activateConstraints:@[
        [grid.topAnchor constraintEqualToAnchor:content.topAnchor constant:20],
        [grid.bottomAnchor constraintEqualToAnchor:content.bottomAnchor constant:-20],
        [grid.leadingAnchor constraintEqualToAnchor:content.leadingAnchor constant:24],
        [grid.trailingAnchor constraintEqualToAnchor:content.trailingAnchor constant:-24]]];
    self.window.contentView = content;
    [NSNotificationCenter.defaultCenter addObserver:self selector:@selector(sync)
        name:NSUserDefaultsDidChangeNotification object:nil];
}
- (void)sync {
    NSUserDefaults *d = NSUserDefaults.standardUserDefaults;
    unsigned int enabled = enabledLanguages();
    BOOL single = !(enabled & (enabled - 1));
    for (NSInteger i = 0; i < 3; i++) [self.languages setSelected:(enabled & (1 << (i + 1))) != 0 forSegment:i];
    self.automatic.enabled = !single;
    self.automatic.state = [d boolForKey:@"AutomaticDetection"] && !single ? NSControlStateValueOn : NSControlStateValueOff;
    self.autocorrect.state = [d boolForKey:@"Autocorrect"] ? NSControlStateValueOn : NSControlStateValueOff;
    self.tabs.state = tabShortcuts() ? NSControlStateValueOn : NSControlStateValueOff;
    [self.meanings selectItemAtIndex:MIN(2, MAX(0, [d integerForKey:@"ChineseMeaningLanguage"]))];
    self.meanings.enabled = (enabled & 8) != 0;
}
- (void)show {
    if (!self.window) [self build];
    [self sync];
    [self.window setContentSize:self.window.contentView.fittingSize];
    [self.window center];
    [self.window makeKeyAndOrderFront:nil];
}
- (void)toggleLanguage:(NSSegmentedControl *)sender {
    unsigned int mask = 0;
    for (NSInteger i = 0; i < 3; i++) if ([sender isSelectedForSegment:i]) mask |= 1 << (i + 1);
    if (!mask) { NSBeep(); [self sync]; return; } // At least one language stays on.
    NSUserDefaults *d = NSUserDefaults.standardUserDefaults;
    [d setInteger:mask forKey:@"EnabledLanguages"];
    if (!(mask & (1 << [d integerForKey:@"Language"]))) [d setInteger:firstLanguage(mask) forKey:@"Language"];
}
- (void)toggleAutomatic:(NSButton *)sender { [NSUserDefaults.standardUserDefaults setBool:sender.state == NSControlStateValueOn forKey:@"AutomaticDetection"]; }
- (void)toggleAutocorrect:(NSButton *)sender { [NSUserDefaults.standardUserDefaults setBool:sender.state == NSControlStateValueOn forKey:@"Autocorrect"]; }
- (void)selectMeanings:(NSPopUpButton *)sender { [NSUserDefaults.standardUserDefaults setInteger:sender.indexOfSelectedItem forKey:@"ChineseMeaningLanguage"]; }
- (void)toggleTabs:(NSButton *)sender { [NSUserDefaults.standardUserDefaults setBool:sender.state == NSControlStateValueOn forKey:@"TabShortcuts"]; }
@end


@interface KeyInController : IMKInputController
@property NSMutableString *raw;
@property NSString *preedit;
@property NSString *language;
@property NSArray<NSString *> *choices;
@property (nonatomic) NSUInteger page;
// Highlighted candidate on the current page; ↑↓ move it, Space / Return commit it.
@property (nonatomic) NSUInteger highlight;
- (NSUInteger)highlightedIndex;
@property NSDictionary<NSString *, NSDictionary *> *meanings;
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
@end
@implementation KeyInController
// Any page change starts again at that page's first candidate.
- (void)setPage:(NSUInteger)page { _page = page; _highlight = 0; }
- (NSUInteger)highlightedIndex { return MIN(self.page * 9 + self.highlight, self.choices.count ? self.choices.count - 1 : 0); }
- (id)initWithServer:(IMKServer *)server delegate:(id)delegate client:(id)client {
    self = [super initWithServer:server delegate:delegate client:client];
    if (self) { _raw = [NSMutableString new]; _choices = @[]; _preedit = @""; _rime = [KeyInRime new]; _recentLanguages = [NSMutableArray new]; }
    return self;
}
- (unsigned int)mode {
    NSUserDefaults *d = NSUserDefaults.standardUserDefaults;
    unsigned int enabled = enabledLanguages();
    // A single enabled language behaves like manual mode.
    if (!(enabled & (enabled - 1))) return firstLanguage(enabled);
    if ([d boolForKey:@"AutomaticDetection"]) return 0;
    unsigned int language = (unsigned int)[d integerForKey:@"Language"];
    return enabled & (1 << language) ? language : firstLanguage(enabled);
}
- (void)activateServer:(id)sender {
    self.language = self.mode ? @[@"English", @"English", @"Vietnamese", @"Chinese"][MIN(self.mode, 3)] : (self.language ?: @"English");
    [self showCandidates:sender];
}
- (void)deactivateServer:(id)sender { [self commitComposition:sender]; [self.recentLanguages removeAllObjects]; if (!self.mode) self.language = @"English"; [self.panel orderOut:nil]; }
- (void)inputControllerWillClose { self.rime = nil; [self.panel orderOut:nil]; }
- (void)hidePalettes { [self.panel orderOut:nil]; }
- (unsigned int)contextLanguage {
    if (self.raw.length && [self.language isEqualToString:@"Chinese"]) return 3;
    if (self.raw.length && [self.language isEqualToString:@"Vietnamese"]) return 2;
    // Retain confirmed context through temporary ambiguous prefixes.
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
    mask &= enabledLanguages();
    NSMutableArray *options = [NSMutableArray new];
    for (unsigned int language = 1; language <= 3; language++)
        if (mask & (1 << language)) [options addObject:@(language)];
    self.languageOptions = options;
}
// Tab rotates the word being typed through EN → VI → 中; manual mode keeps the choice.
- (void)switchLanguage:(id<IMKTextInput>)client {
    NSUInteger active = [@[@"English", @"Vietnamese", @"Chinese"] indexOfObject:self.language];
    if (active == NSNotFound) active = 0;
    unsigned int enabled = enabledLanguages(), next = (unsigned int)active + 1;
    do next = next % 3 + 1; while (!(enabled & (1 << next)));
    if (next == active + 1) return;
    if (self.mode) [NSUserDefaults.standardUserDefaults setInteger:next forKey:@"Language"];
    self.wordMode = next;
    self.page = 0; self.rimeEditing = NO; self.usingRime = NO;
    [self.rime clear];
    [self refresh:client];
}
// Shift+Tab toggles automatic detection; turning it off pins the language currently shown.
- (void)toggleAutomaticWhileTyping:(id<IMKTextInput>)client {
    NSUserDefaults *d = NSUserDefaults.standardUserDefaults;
    unsigned int enabled = enabledLanguages();
    if (!(enabled & (enabled - 1))) return;
    if (self.mode) {
        [d setBool:YES forKey:@"AutomaticDetection"];
        self.wordMode = 0;
    } else {
        NSUInteger active = [@[@"English", @"Vietnamese", @"Chinese"] indexOfObject:self.language];
        [d setInteger:active == NSNotFound ? 1 : active + 1 forKey:@"Language"];
        [d setBool:NO forKey:@"AutomaticDetection"];
    }
    [self.recentLanguages removeAllObjects];
    self.page = 0; self.rimeEditing = NO; self.usingRime = NO;
    [self.rime clear];
    [self refresh:client];
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
    NSUInteger detected = [@[@"English", @"Vietnamese", @"Chinese"] indexOfObject:lines[0]];
    unsigned int enabled = enabledLanguages();
    if (!effectiveMode && detected != NSNotFound && !(enabled & (1 << (detected + 1)))) {
        // Detection picked a disabled language: fall back to the best enabled guess.
        unsigned int options = 0;
        for (NSNumber *option in self.languageOptions) options |= 1 << option.unsignedIntValue;
        effectiveMode = firstLanguage(options ?: enabled);
        if (effectiveMode == 3 && self.rime.available && [self.rime setInput:self.raw]) {
            self.usingRime = YES; [self renderRime:client]; return;
        }
        result = keyin_compose_context(self.raw.UTF8String, effectiveMode, [self contextLanguage]);
        if (!result) return;
        lines = [[NSString stringWithUTF8String:result] componentsSeparatedByString:@"\n"];
        keyin_free(result);
    }
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
static NSString *shortMeaning(NSString *gloss) {
    NSString *first = [[gloss componentsSeparatedByString:@";"] firstObject];
    first = [first stringByTrimmingCharactersInSet:NSCharacterSet.whitespaceAndNewlineCharacterSet];
    if (first.length > 72) {
        NSRange range = [first rangeOfComposedCharacterSequencesForRange:NSMakeRange(0, 69)];
        first = [[first substringWithRange:range] stringByAppendingString:@"…"];
    }
    return first;
}
// Keep whole-word glosses; otherwise annotate individual words/characters, not a sentence translation.
- (NSString *)meaningForCandidate:(NSString *)candidate {
    NSInteger mode = [NSUserDefaults.standardUserDefaults integerForKey:@"ChineseMeaningLanguage"];
    if (mode != 1 && mode != 2) return nil;
    if (!self.meanings) {
        // Share the large immutable dictionary across all app input controllers.
        static NSDictionary *sharedMeanings;
        static dispatch_once_t once;
        dispatch_once(&once, ^{
            NSURL *url = [NSBundle.mainBundle URLForResource:@"chinese-meanings" withExtension:@"json"];
            NSData *data = url ? [NSData dataWithContentsOfURL:url] : nil;
            sharedMeanings = data ? [NSJSONSerialization JSONObjectWithData:data options:0 error:nil] : @{};
        });
        self.meanings = sharedMeanings;
    }
    NSString *key = mode == 1 ? @"vi" : @"en";
    NSString *meaning = self.meanings[candidate][key];
    if (meaning.length) return shortMeaning(meaning);
    NSMutableArray<NSString *> *characters = [NSMutableArray new];
    [candidate enumerateSubstringsInRange:NSMakeRange(0, candidate.length)
        options:NSStringEnumerationByComposedCharacterSequences
        usingBlock:^(NSString *substring, NSRange range, NSRange enclosingRange, BOOL *stop) {
            [characters addObject:substring];
        }];
    NSMutableArray<NSString *> *parts = [NSMutableArray new];
    for (NSUInteger offset = 0; offset < characters.count;) {
        BOOL found = NO;
        // Longest dictionary word first; a single character is the final fallback.
        for (NSUInteger length = characters.count - offset; length > 0; length--) {
            NSString *word = [[characters subarrayWithRange:NSMakeRange(offset, length)] componentsJoinedByString:@""];
            NSString *gloss = self.meanings[word][key];
            if (!gloss.length) continue;
            // Short first senses keep sentence annotations readable while typing.
            NSString *shortGloss = shortMeaning(gloss);
            [parts addObject:[NSString stringWithFormat:@"%@: %@", word, shortGloss]];
            offset += length;
            found = YES;
            break;
        }
        if (!found) offset++; // Unknown characters/punctuation have no placeholder.
    }
    return parts.count ? [parts componentsJoinedByString:@" · "] : nil;
}
- (NSString *)candidateLabelAtIndex:(NSUInteger)index number:(NSUInteger)number {
    NSString *candidate = self.choices[index];
    NSString *meaning = [self.language isEqualToString:@"Chinese"] ? [self meaningForCandidate:candidate] : nil;
    return meaning ? [NSString stringWithFormat:@"%lu %@ — %@", number, candidate, meaning] :
        [NSString stringWithFormat:@"%lu %@", number, candidate];
}
- (NSAttributedString *)styledCandidateLabelAtIndex:(NSUInteger)index number:(NSUInteger)number selected:(BOOL)selected {
    NSString *text = [self candidateLabelAtIndex:index number:number];
    NSColor *primary = selected ? NSColor.whiteColor : NSColor.labelColor;
    NSColor *secondary = selected ? [NSColor colorWithWhite:1 alpha:0.8] : NSColor.secondaryLabelColor;
    NSMutableAttributedString *label = [[NSMutableAttributedString alloc] initWithString:text
        attributes:@{NSFontAttributeName: [NSFont systemFontOfSize:16 weight:NSFontWeightRegular],
            NSForegroundColorAttributeName: primary}];
    // Muted index digits, like the system candidate bar.
    NSUInteger numberLength = [NSString stringWithFormat:@"%lu", number].length;
    [label addAttributes:@{NSFontAttributeName: [NSFont monospacedDigitSystemFontOfSize:15 weight:NSFontWeightRegular],
        NSForegroundColorAttributeName: secondary} range:NSMakeRange(0, numberLength)];
    NSUInteger prefixLength = [NSString stringWithFormat:@"%lu %@", number, self.choices[index]].length;
    if (text.length > prefixLength) {
        NSFont *meaningFont = [NSFont systemFontOfSize:13 weight:NSFontWeightLight];
        NSFont *italic = [NSFontManager.sharedFontManager convertFont:meaningFont toHaveTrait:NSItalicFontMask];
        NSMutableDictionary *attributes = [@{NSFontAttributeName: italic,
            NSForegroundColorAttributeName: secondary} mutableCopy];
        if (!(italic.fontDescriptor.symbolicTraits & NSFontItalicTrait)) attributes[NSObliquenessAttributeName] = @0.18;
        [label addAttributes:attributes range:NSMakeRange(prefixLength, text.length - prefixLength)];
    }
    return label;
}
static NSImage *roundedMask(CGFloat radius) {
    CGFloat side = radius * 2 + 1;
    NSImage *mask = [NSImage imageWithSize:NSMakeSize(side, side) flipped:NO drawingHandler:^BOOL(NSRect rect) {
        [NSColor.blackColor setFill];
        [[NSBezierPath bezierPathWithRoundedRect:rect xRadius:radius yRadius:radius] fill];
        return YES;
    }];
    mask.capInsets = NSEdgeInsetsMake(radius, radius, radius, radius);
    mask.resizingMode = NSImageResizingModeStretch;
    return mask;
}
- (void)showCandidates:(id<IMKTextInput>)client {
    const CGFloat inset = 6, radius = 14, itemHeight = 34, itemPadding = 12, gap = 4, footerHeight = 30, chevronWidth = 26;
    BOOL hasChoices = self.choices.count > 0;
    NSUInteger pages = (self.choices.count + 8) / 9;
    NSUInteger start = self.page * 9;
    NSUInteger count = MIN(9, self.choices.count - MIN(start, self.choices.count));
    NSInteger meaningMode = [NSUserDefaults.standardUserDefaults integerForKey:@"ChineseMeaningLanguage"];
    BOOL showMeanings = hasChoices && [self.language isEqualToString:@"Chinese"] && (meaningMode == 1 || meaningMode == 2);

    NSString *language = self.correctionLanguage ?: self.language ?: @"English";
    BOOL vi = [language isEqualToString:@"Vietnamese"];
    BOOL zh = [language isEqualToString:@"Chinese"];
    NSString *badge = vi ? @"🇻🇳 VI" : (zh ? @"🇨🇳 简中" : @"🇺🇸 EN");
    // Footer: coloured language badge first, then muted key hints.
    NSMutableArray<NSString *> *hints = [NSMutableArray arrayWithObject:self.wordMode ? @"Chọn" : (self.mode ? @"Manual" : @"Auto")];
    if (self.correction) {
        [hints addObject:[NSString stringWithFormat:@"Space → %@", self.correction]];
        [hints addObject:@"Esc giữ gốc"];
    } else if (hasChoices) {
        if (pages > 1) [hints addObject:[NSString stringWithFormat:@"%lu/%lu", self.page + 1, pages]];
        [hints addObject:@"↑↓ 1–9 chọn"];
        [hints addObject:@"Space nhập"];
        if (pages > 1) [hints addObject:@"← → trang"];
    }
    if (self.raw.length && tabShortcuts()) [hints addObjectsFromArray:@[@"Tab đổi tiếng", @"⇧Tab Auto"]];
    NSMutableAttributedString *statusText = [[NSMutableAttributedString alloc] initWithString:badge attributes:@{
        NSFontAttributeName: [NSFont systemFontOfSize:13 weight:NSFontWeightSemibold],
        NSForegroundColorAttributeName: vi ? NSColor.systemTealColor : (zh ? NSColor.systemOrangeColor : NSColor.systemBlueColor)}];
    [statusText appendAttributedString:[[NSAttributedString alloc] initWithString:[@"     " stringByAppendingString:[hints componentsJoinedByString:@"     "]] attributes:@{
        NSFontAttributeName: [NSFont systemFontOfSize:13 weight:NSFontWeightRegular],
        NSForegroundColorAttributeName: NSColor.secondaryLabelColor}]];
    CGFloat statusWidth = ceil(statusText.size.width) + 2 * itemPadding + 4;

    NSMutableArray<NSAttributedString *> *candidateLabels = [NSMutableArray new];
    NSMutableArray<NSValue *> *itemFrames = [NSMutableArray new];
    CGFloat width, height;
    BOOL more = pages > 1;
    if (showMeanings) {
        width = MAX(520, statusWidth); // Rows span the panel even when the footer widens it.
        CGFloat y = footerHeight + inset;
        CGFloat textWidth = width - 2 * inset - 2 * itemPadding;
        // Lay rows out bottom-up so the first candidate ends on top.
        NSMutableArray<NSNumber *> *rowHeights = [NSMutableArray new];
        for (NSUInteger i = 0; i < count; i++) {
            NSAttributedString *text = [self styledCandidateLabelAtIndex:start + i number:i + 1 selected:i == self.highlight];
            [candidateLabels addObject:text];
            NSRect bounds = [text boundingRectWithSize:NSMakeSize(textWidth, CGFLOAT_MAX) options:NSStringDrawingUsesLineFragmentOrigin];
            [rowHeights addObject:@(MAX(itemHeight, MIN(70, ceil(bounds.size.height) + 12)))];
        }
        for (NSInteger i = count - 1; i >= 0; i--) {
            CGFloat rowHeight = rowHeights[i].doubleValue;
            [itemFrames insertObject:[NSValue valueWithRect:NSMakeRect(inset, y, width - 2 * inset, rowHeight)] atIndex:0];
            y += rowHeight + (i ? 2 : 0);
        }
        height = y + inset;
    } else {
        CGFloat x = inset;
        for (NSUInteger i = 0; i < count; i++) {
            NSAttributedString *text = [self styledCandidateLabelAtIndex:start + i number:i + 1 selected:i == self.highlight];
            [candidateLabels addObject:text];
            CGFloat itemWidth = ceil(text.size.width) + 2 * itemPadding;
            [itemFrames addObject:[NSValue valueWithRect:NSMakeRect(x, footerHeight + inset, itemWidth, itemHeight)]];
            x += itemWidth + gap;
        }
        width = hasChoices ? x - gap + (more ? chevronWidth : 0) + inset : 0;
        height = footerHeight + (hasChoices ? itemHeight + 2 * inset : 0);
    }
    width = MAX(width, statusWidth);

    if (!self.panel) {
        self.panel = [[NSPanel alloc] initWithContentRect:NSMakeRect(0, 0, width, height) styleMask:NSWindowStyleMaskBorderless | NSWindowStyleMaskNonactivatingPanel backing:NSBackingStoreBuffered defer:NO];
        self.panel.level = NSPopUpMenuWindowLevel;
        self.panel.hasShadow = YES;
        self.panel.opaque = NO;
        self.panel.backgroundColor = NSColor.clearColor;
        self.panel.ignoresMouseEvents = YES;
        // Frosted glass surface that follows the system light/dark appearance.
        NSVisualEffectView *surface = [[NSVisualEffectView alloc] initWithFrame:NSMakeRect(0, 0, width, height)];
        surface.material = NSVisualEffectMaterialPopover;
        surface.blendingMode = NSVisualEffectBlendingModeBehindWindow;
        surface.state = NSVisualEffectStateActive;
        surface.maskImage = roundedMask(radius);
        self.panel.contentView = surface;
        self.status = [NSTextField labelWithString:@""];
        self.status.lineBreakMode = NSLineBreakByTruncatingTail;
        [surface addSubview:self.status];
    }
    [self.panel setContentSize:NSMakeSize(width, height)];
    NSView *surface = self.panel.contentView;
    for (NSView *view in surface.subviews.copy)
        if (view != self.status) [view removeFromSuperview];

    self.status.attributedStringValue = statusText;
    CGFloat statusTextHeight = ceil(statusText.size.height);
    self.status.frame = NSMakeRect(itemPadding - 2, floor((footerHeight - statusTextHeight) / 2), width - 2 * itemPadding + 4, statusTextHeight);
    self.panel.accessibilityLabel = statusText.string;
    if (hasChoices) {
        NSBox *divider = [[NSBox alloc] initWithFrame:NSMakeRect(0, footerHeight, width, 1)];
        divider.boxType = NSBoxSeparator;
        [surface addSubview:divider];
    }

    for (NSUInteger i = 0; i < count; i++) {
        NSRect frame = itemFrames[i].rectValue;
        NSView *item = [[NSView alloc] initWithFrame:frame];
        if (i == self.highlight) {
            item.wantsLayer = YES;
            item.layer.cornerRadius = 9;
            item.layer.backgroundColor = NSColor.controlAccentColor.CGColor;
        }
        NSTextField *label = [NSTextField labelWithString:@""];
        label.attributedStringValue = candidateLabels[i];
        label.lineBreakMode = NSLineBreakByTruncatingTail;
        label.maximumNumberOfLines = 1;
        label.toolTip = label.stringValue;
        label.accessibilityLabel = label.stringValue;
        CGFloat textHeight = ceil(candidateLabels[i].size.height);
        if (showMeanings) {
            label.lineBreakMode = NSLineBreakByWordWrapping;
            label.maximumNumberOfLines = 3;
            label.cell.wraps = YES;
            label.cell.scrollable = NO;
            textHeight = frame.size.height - 10;
        }
        // The text cell pads 2pt per side, so give it that slack around the measured width.
        label.frame = NSMakeRect(itemPadding - 2, floor((frame.size.height - textHeight) / 2), frame.size.width - 2 * itemPadding + 4, textHeight);
        [item addSubview:label];
        [surface addSubview:item];
    }
    if (hasChoices && more && !showMeanings) {
        NSImage *chevron = [NSImage imageWithSystemSymbolName:@"chevron.right" accessibilityDescription:@"More candidates"];
        NSImageView *next = [NSImageView imageViewWithImage:[chevron imageWithSymbolConfiguration:
            [NSImageSymbolConfiguration configurationWithPointSize:13 weight:NSFontWeightSemibold]]];
        next.contentTintColor = self.page + 1 < pages ? NSColor.labelColor : NSColor.tertiaryLabelColor;
        next.frame = NSMakeRect(width - inset - chevronWidth, footerHeight + inset, chevronWidth, itemHeight);
        [surface addSubview:next];
    }

    NSRect caret = NSZeroRect;
    [client attributesForCharacterIndex:0 lineHeightRectangle:&caret];
    NSScreen *screen = NSScreen.mainScreen;
    for (NSScreen *s in NSScreen.screens) if (NSPointInRect(caret.origin, s.frame)) { screen = s; break; }
    NSRect bounds = screen.visibleFrame;
    CGFloat x = MAX(NSMinX(bounds), MIN(caret.origin.x - inset - itemPadding, NSMaxX(bounds) - width));
    CGFloat y = caret.origin.y - height - 6;
    if (y < NSMinY(bounds)) y = NSMaxY(caret) + 6;
    y = MAX(NSMinY(bounds), MIN(y, NSMaxY(bounds) - height));
    [self.panel setFrameOrigin:NSMakePoint(x, y)];
    [self.panel invalidateShadow];
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
    if (flags & (NSEventModifierFlagCommand | NSEventModifierFlagControl | NSEventModifierFlagOption)) { [self commitComposition:client]; return NO; }
    // Only claim Tab while a word is being composed; otherwise apps keep their own Tab.
    if (event.keyCode == kVK_Tab && self.raw.length) {
        if (!tabShortcuts()) { [self commitComposition:client]; return NO; }
        if (flags & NSEventModifierFlagShift) [self toggleAutomaticWhileTyping:client];
        else [self switchLanguage:client];
        return YES;
    }
    if (!(flags & NSEventModifierFlagShift) && (event.keyCode == kVK_UpArrow || event.keyCode == kVK_DownArrow)) {
        if (self.choices.count) {
            NSUInteger count = self.choices.count;
            NSUInteger next = ([self highlightedIndex] + (event.keyCode == kVK_DownArrow ? 1 : count - 1)) % count;
            self.page = next / 9; self.highlight = next % 9;
            [self showCandidates:client]; return YES;
        }
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
    if (event.keyCode == kVK_Space || event.keyCode == kVK_Return) {
        if (!self.raw.length) return NO;
        if (self.usingRime && self.choices.count) {
            [self selectRimeCandidate:[self highlightedIndex] client:client];
            return YES;
        }
        BOOL chinese = self.choices.count > 0 || [self.correctionLanguage isEqualToString:@"Chinese"];
        NSString *value = self.correction ?: (chinese ? self.choices[[self highlightedIndex]] : ([self.language isEqualToString:@"Chinese"] ? self.raw.copy : self.preedit));
        if (self.correctionLanguage) self.language = self.correctionLanguage;
        [self rememberLanguage];
        [self finish:value client:client];
        return chinese;
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
- (void)showPreferences:(id)sender { [KeyInSettings.shared show]; }
- (NSMenu *)menu {
    NSMenu *menu = [NSMenu new];
    [menu addItemWithTitle:@"Tùy chỉnh…" action:@selector(showPreferences:) keyEquivalent:@","];
    return menu;
}
@end
int main(void) {
    @autoreleasepool {
        [NSApplication sharedApplication];
        [NSUserDefaults.standardUserDefaults registerDefaults:@{@"AutomaticDetection": @YES, @"Autocorrect": @YES, @"LearnChinese": @YES, @"Language": @1, @"ChineseMeaningLanguage": @0, @"EnabledLanguages": @14, @"TabShortcuts": @YES}];
        NSString *shared = [NSBundle.mainBundle.resourcePath stringByAppendingPathComponent:@"rime"];
        NSString *user = [NSHomeDirectory() stringByAppendingPathComponent:@"Library/Application Support/KeyIn/Rime"];
        [KeyInRime startWithSharedData:shared userData:user];
        server = [[IMKServer alloc] initWithName:@"KeyIn_Connection" bundleIdentifier:NSBundle.mainBundle.bundleIdentifier];
        if (!server) return 1;
        [NSApp run];
    }
    return 0;
}
