import 'dart:async';

import 'package:flutter/foundation.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_localizations/flutter_localizations.dart';
import 'package:intl/intl.dart' as intl;

import 'app_localizations_en.dart';
import 'app_localizations_zh.dart';

// ignore_for_file: type=lint

/// Callers can lookup localized strings with an instance of AppLocalizations
/// returned by `AppLocalizations.of(context)`.
///
/// Applications need to include `AppLocalizations.delegate()` in their app's
/// `localizationDelegates` list, and the locales they support in the app's
/// `supportedLocales` list. For example:
///
/// ```dart
/// import 'l10n/app_localizations.dart';
///
/// return MaterialApp(
///   localizationsDelegates: AppLocalizations.localizationsDelegates,
///   supportedLocales: AppLocalizations.supportedLocales,
///   home: MyApplicationHome(),
/// );
/// ```
///
/// ## Update pubspec.yaml
///
/// Please make sure to update your pubspec.yaml to include the following
/// packages:
///
/// ```yaml
/// dependencies:
///   # Internationalization support.
///   flutter_localizations:
///     sdk: flutter
///   intl: any # Use the pinned version from flutter_localizations
///
///   # Rest of dependencies
/// ```
///
/// ## iOS Applications
///
/// iOS applications define key application metadata, including supported
/// locales, in an Info.plist file that is built into the application bundle.
/// To configure the locales supported by your app, you’ll need to edit this
/// file.
///
/// First, open your project’s ios/Runner.xcworkspace Xcode workspace file.
/// Then, in the Project Navigator, open the Info.plist file under the Runner
/// project’s Runner folder.
///
/// Next, select the Information Property List item, select Add Item from the
/// Editor menu, then select Localizations from the pop-up menu.
///
/// Select and expand the newly-created Localizations item then, for each
/// locale your application supports, add a new item and select the locale
/// you wish to add from the pop-up menu in the Value field. This list should
/// be consistent with the languages listed in the AppLocalizations.supportedLocales
/// property.
abstract class AppLocalizations {
  AppLocalizations(String locale)
    : localeName = intl.Intl.canonicalizedLocale(locale.toString());

  final String localeName;

  static AppLocalizations? of(BuildContext context) {
    return Localizations.of<AppLocalizations>(context, AppLocalizations);
  }

  static const LocalizationsDelegate<AppLocalizations> delegate =
      _AppLocalizationsDelegate();

  /// A list of this localizations delegate along with the default localizations
  /// delegates.
  ///
  /// Returns a list of localizations delegates containing this delegate along with
  /// GlobalMaterialLocalizations.delegate, GlobalCupertinoLocalizations.delegate,
  /// and GlobalWidgetsLocalizations.delegate.
  ///
  /// Additional delegates can be added by appending to this list in
  /// MaterialApp. This list does not have to be used at all if a custom list
  /// of delegates is preferred or required.
  static const List<LocalizationsDelegate<dynamic>> localizationsDelegates =
      <LocalizationsDelegate<dynamic>>[
        delegate,
        GlobalMaterialLocalizations.delegate,
        GlobalCupertinoLocalizations.delegate,
        GlobalWidgetsLocalizations.delegate,
      ];

  /// A list of this localizations delegate's supported locales.
  static const List<Locale> supportedLocales = <Locale>[
    Locale('en'),
    Locale('zh'),
  ];

  /// No description provided for @memo.
  ///
  /// In en, this message translates to:
  /// **'Memo'**
  String get memo;

  /// No description provided for @updateMemo.
  ///
  /// In en, this message translates to:
  /// **'Update Memo'**
  String get updateMemo;

  /// No description provided for @addMemo.
  ///
  /// In en, this message translates to:
  /// **'Add Memo'**
  String get addMemo;

  /// No description provided for @word.
  ///
  /// In en, this message translates to:
  /// **'Word'**
  String get word;

  /// No description provided for @displayMode.
  ///
  /// In en, this message translates to:
  /// **'Display Mode'**
  String get displayMode;

  /// No description provided for @displayModeAuto.
  ///
  /// In en, this message translates to:
  /// **'Auto'**
  String get displayModeAuto;

  /// No description provided for @displayModeText.
  ///
  /// In en, this message translates to:
  /// **'Text'**
  String get displayModeText;

  /// No description provided for @displayModeImage.
  ///
  /// In en, this message translates to:
  /// **'Image'**
  String get displayModeImage;

  /// No description provided for @displayAll.
  ///
  /// In en, this message translates to:
  /// **'All'**
  String get displayAll;

  /// No description provided for @imageConvertFailure.
  ///
  /// In en, this message translates to:
  /// **'Image Convert Failure'**
  String get imageConvertFailure;

  /// No description provided for @convertToBase64.
  ///
  /// In en, this message translates to:
  /// **'Convert To Base64'**
  String get convertToBase64;

  /// No description provided for @toggleDatetime.
  ///
  /// In en, this message translates to:
  /// **'Toggle Datetime'**
  String get toggleDatetime;

  /// No description provided for @toggleOnlineMemo.
  ///
  /// In en, this message translates to:
  /// **'Toggle Online Memo'**
  String get toggleOnlineMemo;

  /// No description provided for @selectFile.
  ///
  /// In en, this message translates to:
  /// **'Please select file'**
  String get selectFile;

  /// No description provided for @importFile.
  ///
  /// In en, this message translates to:
  /// **'Import file'**
  String get importFile;

  /// No description provided for @setting.
  ///
  /// In en, this message translates to:
  /// **'Setting'**
  String get setting;

  /// No description provided for @scan.
  ///
  /// In en, this message translates to:
  /// **'Scan'**
  String get scan;

  /// No description provided for @add.
  ///
  /// In en, this message translates to:
  /// **'Add'**
  String get add;

  /// No description provided for @confirm.
  ///
  /// In en, this message translates to:
  /// **'Confirm'**
  String get confirm;

  /// No description provided for @confirmDelete.
  ///
  /// In en, this message translates to:
  /// **'Delete this item?'**
  String get confirmDelete;

  /// No description provided for @cancel.
  ///
  /// In en, this message translates to:
  /// **'Cancel'**
  String get cancel;

  /// No description provided for @delete.
  ///
  /// In en, this message translates to:
  /// **'Delete'**
  String get delete;

  /// No description provided for @save.
  ///
  /// In en, this message translates to:
  /// **'Save'**
  String get save;

  /// No description provided for @about.
  ///
  /// In en, this message translates to:
  /// **'About'**
  String get about;

  /// No description provided for @version.
  ///
  /// In en, this message translates to:
  /// **'Version'**
  String get version;

  /// No description provided for @newVersionDownload.
  ///
  /// In en, this message translates to:
  /// **'New Version Downloading'**
  String get newVersionDownload;

  /// No description provided for @newVersionUpdate.
  ///
  /// In en, this message translates to:
  /// **'New Version Update'**
  String get newVersionUpdate;

  /// No description provided for @versionUpToDate.
  ///
  /// In en, this message translates to:
  /// **'Up To Date'**
  String get versionUpToDate;

  /// No description provided for @newVersion.
  ///
  /// In en, this message translates to:
  /// **'New'**
  String get newVersion;

  /// No description provided for @planTime.
  ///
  /// In en, this message translates to:
  /// **'Estimated time'**
  String get planTime;

  /// No description provided for @inputContentHint.
  ///
  /// In en, this message translates to:
  /// **'Please type something'**
  String get inputContentHint;

  /// No description provided for @qrcodeNotShow.
  ///
  /// In en, this message translates to:
  /// **'The qrcode not show'**
  String get qrcodeNotShow;

  /// A message with word count
  ///
  /// In en, this message translates to:
  /// **'Text to long({current}/{max} Words)'**
  String qrcodeContentToLong(int current, int max);

  /// No description provided for @settingsTextScaling.
  ///
  /// In en, this message translates to:
  /// **'Text scaling'**
  String get settingsTextScaling;

  /// No description provided for @settingsTextScalingSmall.
  ///
  /// In en, this message translates to:
  /// **'Small'**
  String get settingsTextScalingSmall;

  /// No description provided for @settingsTextScalingNormal.
  ///
  /// In en, this message translates to:
  /// **'Normal'**
  String get settingsTextScalingNormal;

  /// No description provided for @settingsTextScalingLarge.
  ///
  /// In en, this message translates to:
  /// **'Large'**
  String get settingsTextScalingLarge;

  /// No description provided for @settingsTextScalingHuge.
  ///
  /// In en, this message translates to:
  /// **'Huge'**
  String get settingsTextScalingHuge;

  /// No description provided for @settingsTheme.
  ///
  /// In en, this message translates to:
  /// **'Theme'**
  String get settingsTheme;

  /// No description provided for @settingsDarkTheme.
  ///
  /// In en, this message translates to:
  /// **'Dark'**
  String get settingsDarkTheme;

  /// No description provided for @settingsLightTheme.
  ///
  /// In en, this message translates to:
  /// **'Light'**
  String get settingsLightTheme;

  /// No description provided for @settingsSystemDefault.
  ///
  /// In en, this message translates to:
  /// **'System'**
  String get settingsSystemDefault;

  /// No description provided for @settingsLocale.
  ///
  /// In en, this message translates to:
  /// **'Locale'**
  String get settingsLocale;

  /// No description provided for @login.
  ///
  /// In en, this message translates to:
  /// **'Login'**
  String get login;

  /// No description provided for @pleaseLogin.
  ///
  /// In en, this message translates to:
  /// **'Please login'**
  String get pleaseLogin;

  /// No description provided for @areYouSureLogout.
  ///
  /// In en, this message translates to:
  /// **'Are you sure logout?'**
  String get areYouSureLogout;

  /// No description provided for @account.
  ///
  /// In en, this message translates to:
  /// **'Account'**
  String get account;

  /// No description provided for @password.
  ///
  /// In en, this message translates to:
  /// **'Password'**
  String get password;

  /// No description provided for @loginSuccess.
  ///
  /// In en, this message translates to:
  /// **'Login success'**
  String get loginSuccess;

  /// No description provided for @loginFailure.
  ///
  /// In en, this message translates to:
  /// **'Login Failure'**
  String get loginFailure;

  /// No description provided for @loginTimeout.
  ///
  /// In en, this message translates to:
  /// **'Login Timeout'**
  String get loginTimeout;

  /// No description provided for @loginNotOpen.
  ///
  /// In en, this message translates to:
  /// **'Login feature not open'**
  String get loginNotOpen;

  /// No description provided for @loadMore.
  ///
  /// In en, this message translates to:
  /// **'Scroll up to load more...'**
  String get loadMore;

  /// No description provided for @loadFail.
  ///
  /// In en, this message translates to:
  /// **'Loading failed, please try again'**
  String get loadFail;

  /// No description provided for @releaseToLoad.
  ///
  /// In en, this message translates to:
  /// **'Release to loading'**
  String get releaseToLoad;

  /// No description provided for @hasNoData.
  ///
  /// In en, this message translates to:
  /// **'No data'**
  String get hasNoData;

  /// No description provided for @hasNoDataClickRefresh.
  ///
  /// In en, this message translates to:
  /// **'No data,click to refresh'**
  String get hasNoDataClickRefresh;

  /// No description provided for @loading.
  ///
  /// In en, this message translates to:
  /// **'Loading ...'**
  String get loading;

  /// No description provided for @successful.
  ///
  /// In en, this message translates to:
  /// **'Load successful'**
  String get successful;

  /// No description provided for @scrollDownToRefresh.
  ///
  /// In en, this message translates to:
  /// **'Scroll down to refresh'**
  String get scrollDownToRefresh;

  /// No description provided for @serverError.
  ///
  /// In en, this message translates to:
  /// **'Service Exception'**
  String get serverError;

  /// No description provided for @search.
  ///
  /// In en, this message translates to:
  /// **'Search'**
  String get search;

  /// No description provided for @inputSearchHint.
  ///
  /// In en, this message translates to:
  /// **'Enter your search term'**
  String get inputSearchHint;

  /// No description provided for @locationServiceDisabled.
  ///
  /// In en, this message translates to:
  /// **'Location services are disabled.'**
  String get locationServiceDisabled;

  /// No description provided for @locationPermissionDenied.
  ///
  /// In en, this message translates to:
  /// **'Location permissions are denied.'**
  String get locationPermissionDenied;

  /// No description provided for @locationPermissionDeniedForever.
  ///
  /// In en, this message translates to:
  /// **'Location permissions are permanently denied, we cannot request permissions.'**
  String get locationPermissionDeniedForever;

  /// No description provided for @runningInBackgroundTip.
  ///
  /// In en, this message translates to:
  /// **'App will continue to receive your location even when you aren\'t using it'**
  String get runningInBackgroundTip;

  /// No description provided for @runningInBackground.
  ///
  /// In en, this message translates to:
  /// **'Running in Background'**
  String get runningInBackground;

  /// No description provided for @deviceId.
  ///
  /// In en, this message translates to:
  /// **'Device Id'**
  String get deviceId;

  /// No description provided for @copyDeviceId.
  ///
  /// In en, this message translates to:
  /// **'Copied device id to Clipboard'**
  String get copyDeviceId;
}

class _AppLocalizationsDelegate
    extends LocalizationsDelegate<AppLocalizations> {
  const _AppLocalizationsDelegate();

  @override
  Future<AppLocalizations> load(Locale locale) {
    return SynchronousFuture<AppLocalizations>(lookupAppLocalizations(locale));
  }

  @override
  bool isSupported(Locale locale) =>
      <String>['en', 'zh'].contains(locale.languageCode);

  @override
  bool shouldReload(_AppLocalizationsDelegate old) => false;
}

AppLocalizations lookupAppLocalizations(Locale locale) {
  // Lookup logic when only language code is specified.
  switch (locale.languageCode) {
    case 'en':
      return AppLocalizationsEn();
    case 'zh':
      return AppLocalizationsZh();
  }

  throw FlutterError(
    'AppLocalizations.delegate failed to load unsupported locale "$locale". This is likely '
    'an issue with the localizations generation tool. Please file an issue '
    'on GitHub with a reproducible sample app and the gen-l10n configuration '
    'that was used.',
  );
}
