object FormMain: TFormMain
  Left = 480
  Top = 137
  Width = 800
  Height = 600
  Color = clBtnFace
  Constraints.MinHeight = 300
  Constraints.MinWidth = 350
  Font.Charset = DEFAULT_CHARSET
  Font.Color = clWindowText
  Font.Height = -11
  Font.Name = 'MS Sans Serif'
  Font.Style = []
  KeyPreview = True
  Menu = MainMenu
  OldCreateOrder = False
  OnCloseQuery = FormCloseQuery
  OnCreate = FormCreate
  OnDblClick = N_FullScreenClick
  OnMouseWheel = FormMouseWheel
  OnResize = FormResize
  PixelsPerInch = 96
  TextHeight = 13
  object SplitterRight: TSplitter
    Left = 267
    Top = 0
    Width = 7
    Height = 542
    Align = alRight
    AutoSnap = False
    MinSize = 10
    ResizeStyle = rsUpdate
    OnMoved = SplitterRightMoved
  end
  object PanelRight: TPanel
    Left = 274
    Top = 0
    Width = 510
    Height = 542
    Align = alRight
    BevelOuter = bvNone
    FullRepaint = False
    ParentBackground = True
    ParentColor = True
    TabOrder = 0
    object PanelControl: TPanel
      Left = 0
      Top = 410
      Width = 510
      Height = 132
      Align = alBottom
      BevelOuter = bvNone
      ParentBackground = True
      ParentColor = True
      TabOrder = 0
      object ImageSpeed: TImage
        Left = 118
        Top = 29
        Width = 134
        Height = 100
        AutoSize = True
        ParentShowHint = False
        ShowHint = True
      end
      object ImageGSensor: TImage
        Left = 260
        Top = 6
        Width = 65
        Height = 65
        ParentShowHint = False
        ShowHint = True
      end
      object ImageAzimut: TImage
        Left = 264
        Top = 72
        Width = 57
        Height = 57
        ParentShowHint = False
        ShowHint = True
      end
      object PanelControl_s1: TPanel
        Left = 4
        Top = 4
        Width = 105
        Height = 48
        BevelOuter = bvNone
        ParentBackground = True
        ParentColor = True
        TabOrder = 3
        object ButtonPlay: TSpeedButton
          Left = 0
          Top = 0
          Width = 48
          Height = 48
          AllowAllUp = True
          GroupIndex = 1
          Caption = 'Play'
          Enabled = False
          NumGlyphs = 4
          OnClick = ButtonPlayClick
        end
        object ButtonStop: TSpeedButton
          Left = 57
          Top = 0
          Width = 48
          Height = 48
          Caption = 'Stop'
          Enabled = False
          NumGlyphs = 2
          OnClick = ButtonStopClick
        end
      end
      object PanelControl_s2: TPanel
        Left = 4
        Top = 64
        Width = 105
        Height = 23
        BevelOuter = bvNone
        ParentBackground = True
        ParentColor = True
        TabOrder = 4
        object ButtonVideoZoomIn: TSpeedButton
          Left = 82
          Top = 0
          Width = 23
          Height = 23
          Caption = '+'
          Enabled = False
          NumGlyphs = 2
          ParentShowHint = False
          ShowHint = True
          OnClick = ButtonVideoZoomOutClick
        end
        object ButtonFramePrev: TSpeedButton
          Left = 0
          Top = 0
          Width = 23
          Height = 23
          Caption = 'B'
          Enabled = False
          NumGlyphs = 2
          ParentShowHint = False
          ShowHint = True
          OnClick = ButtonFramePrevClick
        end
        object ButtonVideoZoomOut: TSpeedButton
          Left = 57
          Top = 0
          Width = 23
          Height = 23
          Caption = '-'
          Enabled = False
          NumGlyphs = 2
          ParentShowHint = False
          ShowHint = True
          OnClick = ButtonVideoZoomOutClick
        end
        object ButtonFrameNext: TSpeedButton
          Left = 25
          Top = 0
          Width = 23
          Height = 23
          Caption = 'N'
          Enabled = False
          NumGlyphs = 2
          ParentShowHint = False
          ShowHint = True
          OnClick = ButtonFrameNextClick
        end
      end
      object PanelPlaySpeed: TPanel
        Left = 4
        Top = 90
        Width = 108
        Height = 17
        Hint = 'Play speed'
        Enabled = False
        ParentBackground = True
        ParentColor = True
        ParentShowHint = False
        ShowHint = True
        TabOrder = 1
      end
      object TrackBarVolume: TTrackBar
        Left = 0
        Top = 108
        Width = 113
        Height = 24
        Hint = 'Volume'
        Max = 100
        ParentShowHint = False
        PageSize = 10
        Frequency = 10
        Position = 100
        ShowHint = True
        TabOrder = 0
        TabStop = False
        ThumbLength = 15
        OnChange = TrackBarVolumeChange
      end
      object CheckBoxLoop: TCheckBox
        Left = 118
        Top = 6
        Width = 105
        Height = 17
        Caption = 'Loop A-B'
        ParentShowHint = False
        ShowHint = True
        TabOrder = 2
        OnClick = N_View_Time_LoopABClick
      end
    end
    object PanelRight2: TPanel
      Left = 0
      Top = 0
      Width = 510
      Height = 410
      Align = alClient
      BevelOuter = bvNone
      ParentBackground = True
      ParentColor = True
      TabOrder = 1
      object SplitterListView: TSplitter
        Left = 0
        Top = 169
        Width = 510
        Height = 7
        Cursor = crVSplit
        Align = alBottom
        AutoSnap = False
        MinSize = 1
        ResizeStyle = rsUpdate
        OnMoved = SplitterListViewMoved
      end
      object WebBrowser: TWebBrowser
        Left = 0
        Top = 176
        Width = 510
        Height = 234
        TabStop = False
        Align = alBottom
        TabOrder = 0
        OnBeforeNavigate2 = WebBrowserBeforeNavigate2
        OnNavigateComplete2 = WebBrowserNavigateComplete2
        OnDocumentComplete = WebBrowserDocumentComplete
        ControlData = {
          4C000000B63400002F1800000000000000000000000000000000000000000000
          000000004C000000000000000000000001000000E0D057007335CF11AE690800
          2B2E126208000000000000004C0000000114020000000000C000000000000046
          8000000000000000000000000000000000000000000000000000000000000000
          00000000000000000100000000000000000000000000000000000000
        }
      end
      object WebBrowserAds: TWebBrowser
        Left = 0
        Top = 0
        Width = 510
        Height = 7
        TabStop = False
        Align = alTop
        TabOrder = 1
        OnProgressChange = WebBrowserAdsProgressChange
        OnDocumentComplete = WebBrowserAdsDocumentComplete
        ControlData = {
          4C000000B6340000B90000000000000000000000000000000000000000000000
          000000004C000000000000000000000001000000E0D057007335CF11AE690800
          2B2E126208000000000000004C0000000114020000000000C000000000000046
          8000000000000000000000000000000000000000000000000000000000000000
          00000000000000000100000000000000000000000000000000000000
        }
      end
      object WebBrowserAnalytics: TWebBrowser
        Left = 0
        Top = 7
        Width = 510
        Height = 7
        TabStop = False
        Align = alTop
        TabOrder = 2
        ControlData = {
          4C000000B6340000B90000000000000000000000000000000000000000000000
          000000004C000000000000000000000001000000E0D057007335CF11AE690800
          2B2E126208000000000000004C0000000114020000000000C000000000000046
          8000000000000000000000000000000000000000000000000000000000000000
          00000000000000000100000000000000000000000000000000000000
        }
      end
      object PanelListView: TPanel
        Left = 0
        Top = 14
        Width = 510
        Height = 155
        Align = alClient
        BevelOuter = bvNone
        ParentBackground = True
        ParentColor = True
        TabOrder = 3
        object ListView: TListView
          Left = 0
          Top = 20
          Width = 510
          Height = 135
          Align = alClient
          Columns = <
            item
              Caption = 'Name'
              Width = 113
            end
            item
              Alignment = taRightJustify
              Caption = 'Duration'
              Width = 45
            end
            item
              Alignment = taRightJustify
              Caption = 'Date and Time'
              Width = 95
            end
            item
              Alignment = taRightJustify
              Caption = 'km'
              Width = 54
            end
            item
              Alignment = taRightJustify
              Caption = 'km/h'
              Width = 38
            end
            item
              Caption = 'File Location'
              Width = 36
            end
            item
              Alignment = taRightJustify
              Caption = 'Mb'
              Width = 43
            end
            item
              Alignment = taRightJustify
              Caption = 'Resolution'
              Width = 62
            end>
          FullDrag = True
          GridLines = True
          HideSelection = False
          MultiSelect = True
          OwnerData = True
          OwnerDraw = True
          ReadOnly = True
          RowSelect = True
          PopupMenu = PopupMenuListView
          SmallImages = ImageListView
          TabOrder = 0
          ViewStyle = vsReport
          OnClick = ListViewClick
          OnColumnClick = ListViewColumnClick
          OnColumnDragged = ListViewColumnDragged
          OnCustomDraw = ListViewCustomDraw
          OnData = ListViewData
          OnDblClick = ListViewDblClick
          OnKeyUp = ListViewKeyUp
          OnSelectItem = ListViewSelectItem
        end
        object PanelFileListMenu: TPanel
          Left = 0
          Top = 0
          Width = 510
          Height = 20
          Align = alTop
          BevelOuter = bvNone
          ParentBackground = True
          ParentColor = True
          TabOrder = 1
        end
      end
    end
  end
  object PanelLeft: TPanel
    Left = 0
    Top = 0
    Width = 267
    Height = 542
    Align = alClient
    BevelOuter = bvNone
    FullRepaint = False
    ParentBackground = True
    ParentColor = True
    TabOrder = 1
    object SplitterBottom: TSplitter
      Left = 0
      Top = 350
      Width = 267
      Height = 7
      Cursor = crVSplit
      Align = alBottom
      AutoSnap = False
      MinSize = 42
      ResizeStyle = rsUpdate
      OnMoved = SplitterRightMoved
    end
    object PanelBottom: TPanel
      Left = 0
      Top = 357
      Width = 267
      Height = 185
      Align = alBottom
      BevelOuter = bvNone
      FullRepaint = False
      ParentBackground = True
      ParentColor = True
      TabOrder = 0
      OnMouseDown = ImageSeekMouseDown
      OnMouseMove = ImageSeekMouseMove
      OnMouseUp = ImageSeekMouseUp
      object PanelGraphicsAndZoom: TPanel
        Left = 0
        Top = 0
        Width = 198
        Height = 185
        Align = alClient
        BevelOuter = bvNone
        TabOrder = 0
        object PanelZoomScroll: TPanel
          Left = 0
          Top = 0
          Width = 198
          Height = 17
          Align = alTop
          BevelOuter = bvNone
          ParentBackground = True
          ParentColor = True
          TabOrder = 0
          object ButtonZoomOut: TSpeedButton
            Left = 0
            Top = 0
            Width = 17
            Height = 17
            Caption = '-'
            Flat = True
            OnClick = ButtonZoomInClick
          end
          object ButtonZoomIn: TSpeedButton
            Left = 120
            Top = 0
            Width = 17
            Height = 17
            Caption = '+'
            Flat = True
            OnClick = ButtonZoomInClick
          end
          object ScrollBarZoom: TScrollBar
            Left = 17
            Top = 0
            Width = 100
            Height = 17
            Align = alCustom
            Ctl3D = False
            LargeChange = 10
            Max = 4
            Min = 1
            PageSize = 0
            ParentCtl3D = False
            Position = 2
            TabOrder = 0
            TabStop = False
            OnChange = ScrollBarZoomChange
          end
        end
        object PanelBottomGraphics: TPanel
          Left = 0
          Top = 17
          Width = 198
          Height = 168
          Align = alClient
          BevelOuter = bvNone
          ParentBackground = True
          ParentColor = True
          PopupMenu = PopupMenuGraphics
          TabOrder = 1
          OnMouseDown = ImageSeekMouseDown
          OnMouseMove = ImageSeekMouseMove
          OnMouseUp = ImageSeekMouseUp
          object ImageSeek: TImage
            Left = 0
            Top = 0
            Width = 198
            Height = 40
            Align = alTop
            PopupMenu = PopupMenuGraphics
            OnMouseDown = ImageSeekMouseDown
            OnMouseMove = ImageSeekMouseMove
            OnMouseUp = ImageSeekMouseUp
          end
        end
      end
      object PanelGraphicsAddon: TPanel
        Left = 198
        Top = 0
        Width = 69
        Height = 185
        Align = alRight
        BevelOuter = bvNone
        TabOrder = 1
      end
    end
    object PanelVideo: TPanel
      Left = 0
      Top = 0
      Width = 267
      Height = 350
      Align = alClient
      BevelOuter = bvNone
      BorderStyle = bsSingle
      Color = clGray
      ParentBackground = False
      PopupMenu = PopupMenuVideoWindow
      TabOrder = 1
      OnDblClick = N_FullScreenClick
      OnMouseDown = VideoWindowWMRMouseDown
      OnMouseMove = VideoWindowWMRMouseMove
      OnMouseUp = VideoWindowWMRMouseUp
      object VideoWindowWMR: TVideoWindow
        Left = 64
        Top = 32
        Width = 329
        Height = 281
        Mode = vmVMR
        VMROptions.Mode = vmrWindowless
        Color = clBlack
        Visible = False
        PopupMenu = PopupMenuVideoWindow
        Align = alCustom
        OnDblClick = N_FullScreenClick
        OnMouseDown = VideoWindowWMRMouseDown
        OnMouseMove = VideoWindowWMRMouseMove
        OnMouseUp = VideoWindowWMRMouseUp
      end
      object VideoWindowEVR: TPanel
        Left = 32
        Top = 192
        Width = 185
        Height = 153
        BevelOuter = bvNone
        TabOrder = 1
        Visible = False
        OnDblClick = N_FullScreenClick
        OnMouseDown = VideoWindowWMRMouseDown
        OnMouseMove = VideoWindowWMRMouseMove
        OnMouseUp = VideoWindowWMRMouseUp
      end
    end
  end
  object FilterGraph: TFilterGraph
    GraphEdit = True
    LinearVolume = True
    OnSelectedFilter = FilterGraphSelectedFilter
    Left = 136
    Top = 120
  end
  object XPManifest: TXPManifest
    Left = 48
    Top = 8
  end
  object Timer: TTimer
    Enabled = False
    Interval = 100
    OnTimer = TimerTimer
    Left = 88
    Top = 8
  end
  object MainMenu: TMainMenu
    AutoHotkeys = maManual
    Left = 8
    Top = 8
    object N_File: TMenuItem
      Caption = 'N_File'
      object N_File_Open: TMenuItem
        Caption = 'N_File_Open'
        ShortCut = 16463
        OnClick = N_File_OpenClick
      end
      object N_FileList_Refresh2: TMenuItem
        Caption = 'N_FileList_Refresh'
        ShortCut = 16466
        OnClick = N_FileList_RefreshClick
      end
      object N_FileList_Clear2: TMenuItem
        Caption = 'N_FileList_Clear'
        ShortCut = 16462
        OnClick = N_FileList_ClearClick
      end
      object N_File_Save_Video: TMenuItem
        Caption = 'N_File_Save_Video'
        ShortCut = 16467
        OnClick = N_File_Save_VideoClick
      end
      object N_File_Repair_Video: TMenuItem
        Caption = 'N_File_Repair_Video'
        OnClick = N_File_Repair_VideoClick
      end
      object N_S1: TMenuItem
        Caption = '-'
      end
      object N_File_Convert_GPS: TMenuItem
        Caption = 'N_File_Convert_GPS'
        ShortCut = 16455
        OnClick = N_File_Convert_GPSClick
      end
      object N_S15: TMenuItem
        Caption = '-'
      end
      object N_SetupDevice: TMenuItem
        Caption = 'N_SetupDevice'
        OnClick = N_SetupDeviceClick
      end
      object N_S2: TMenuItem
        Caption = '-'
      end
      object N_Settings_Clear: TMenuItem
        Caption = 'N_Settings_Clear'
        OnClick = N_Settings_ClearClick
      end
      object N_S14: TMenuItem
        Caption = '-'
      end
      object N_File_Exit: TMenuItem
        Caption = 'N_File_Exit'
        ShortCut = 32883
        OnClick = N_File_ExitClick
      end
    end
    object N_View: TMenuItem
      Caption = 'N_View'
      object N_View_Language: TMenuItem
        Caption = 'N_View_Language'
      end
      object N_View_Profile: TMenuItem
        Caption = 'N_View_Profile'
        object N_View_Profile_GPS_G: TMenuItem
          Caption = 'N_View_Profile_GPS_G'
          Checked = True
          RadioItem = True
          OnClick = N_View_Profile_GPSClick
        end
        object N_View_Profile_GPS: TMenuItem
          Caption = 'N_View_Profile_GPS'
          RadioItem = True
          OnClick = N_View_Profile_GPSClick
        end
        object N_View_Profile_Video: TMenuItem
          Caption = 'N_View_Profile_Video'
          RadioItem = True
          OnClick = N_View_Profile_GPSClick
        end
      end
      object N_S18: TMenuItem
        Caption = '-'
      end
      object N_View_FileList: TMenuItem
        Caption = 'N_View_FileList'
        Checked = True
        ShortCut = 113
        OnClick = N_View_FileListClick
      end
      object N_S11: TMenuItem
        Caption = '-'
      end
      object N_View_Maps: TMenuItem
        Caption = 'N_View_Maps'
        object N_View_OpenStreetMaps2: TMenuItem
          Caption = 'N_View_OpenStreetMaps2'
          Checked = True
          RadioItem = True
          OnClick = N_View_GoogleMapsClick
        end
        object N_View_OpenStreetMaps: TMenuItem
          Caption = 'N_View_OpenStreetMaps'
          RadioItem = True
          OnClick = N_View_GoogleMapsClick
        end
        object N_View_GoogleMaps: TMenuItem
          Caption = 'N_View_GoogleMaps'
          RadioItem = True
          ShortCut = 114
          OnClick = N_View_GoogleMapsClick
        end
        object N_View_YandexMaps: TMenuItem
          Caption = 'N_View_YandexMaps'
          RadioItem = True
          OnClick = N_View_GoogleMapsClick
        end
        object N_View_YandexMaps2: TMenuItem
          Caption = 'N_View_YandexMaps2'
          RadioItem = True
          OnClick = N_View_GoogleMapsClick
        end
        object N_View_NavitelMaps: TMenuItem
          Caption = 'N_View_NavitelMaps'
          RadioItem = True
          OnClick = N_View_GoogleMapsClick
        end
        object N_View_BaiduMaps: TMenuItem
          Caption = 'N_View_BaiduMaps'
          RadioItem = True
          OnClick = N_View_GoogleMapsClick
        end
        object N_View_NoMaps: TMenuItem
          Caption = 'N_View_NoMaps'
          RadioItem = True
          OnClick = N_View_GoogleMapsClick
        end
        object N_S20: TMenuItem
          Caption = '-'
        end
        object N_View_Maps_PostAction: TMenuItem
          Caption = 'N_View_Maps_PostAction'
          OnClick = N_View_Maps_PostActionClick
        end
        object N_View_Maps_OpenKML: TMenuItem
          Caption = 'N_View_Maps_OpenKML'
          OnClick = N_View_Maps_OpenKMLClick
        end
      end
      object N_View_Arrow: TMenuItem
        Caption = 'N_View_Arrow'
        object N_View_Arrow_Small: TMenuItem
          Caption = 'N_View_Arrow_Small'
          GroupIndex = 1
          RadioItem = True
          OnClick = N_View_Arrow_SmallClick
        end
        object N_View_Arrow_Normal: TMenuItem
          Caption = 'N_View_Arrow_Normal'
          GroupIndex = 1
          RadioItem = True
          OnClick = N_View_Arrow_SmallClick
        end
        object N_View_Arrow_Big: TMenuItem
          Caption = 'N_View_Arrow_Big'
          Checked = True
          GroupIndex = 1
          RadioItem = True
          OnClick = N_View_Arrow_SmallClick
        end
      end
      object N_View_TimePoints: TMenuItem
        Caption = 'N_View_TimePoints'
        object N_View_TimePoints_1m: TMenuItem
          Caption = 'N_View_TimePoints_1m'
          RadioItem = True
          OnClick = N_View_TimePoints_1mClick
        end
        object N_View_TimePoints_5m: TMenuItem
          Caption = 'N_View_TimePoints_5m'
          RadioItem = True
          OnClick = N_View_TimePoints_1mClick
        end
        object N_View_TimePoints_20m: TMenuItem
          Caption = 'N_View_TimePoints_20m'
          RadioItem = True
          OnClick = N_View_TimePoints_1mClick
        end
        object N_View_TimePoints_Never: TMenuItem
          Caption = 'N_View_TimePoints_Never'
          Checked = True
          RadioItem = True
          OnClick = N_View_TimePoints_1mClick
        end
      end
      object N_View_Coordinates: TMenuItem
        Caption = 'N_View_Coordinates'
        object N_View_Coord_FlipX: TMenuItem
          Caption = 'N_View_Coord_FlipX'
          OnClick = N_View_Coord_FlipXClick
        end
        object N_View_Coord_FlipY: TMenuItem
          Caption = 'N_View_Coord_FlipY'
          OnClick = N_View_Coord_FlipXClick
        end
        object N_S23: TMenuItem
          Caption = '-'
        end
        object N_View_Coord_ShiftX: TMenuItem
          Caption = 'N_View_Coord_ShiftX'
          OnClick = N_View_Coord_ShiftXClick
        end
        object N_View_Coord_ShiftY: TMenuItem
          Caption = 'N_View_Coord_ShiftY'
          OnClick = N_View_Coord_ShiftXClick
        end
        object N_S24: TMenuItem
          Caption = '-'
        end
        object N_View_Coord_Speed: TMenuItem
          Caption = 'N_View_Coord_Speed'
          OnClick = N_View_Coord_FlipXClick
        end
      end
      object N_View_UseCoordinates: TMenuItem
        Caption = 'N_View_UseCoordinates'
        object N_View_MarkerURLSetup: TMenuItem
          Caption = 'N_View_MarkerURLSetup'
          OnClick = N_View_MarkerURLSetupClick
        end
        object N_View_MarkerURLOpen: TMenuItem
          Caption = 'N_View_MarkerURLOpen'
          ShortCut = 77
          OnClick = N_View_MarkerURLOpenClick
        end
        object N_S25: TMenuItem
          Caption = '-'
        end
        object N_View_Coord_CopySetup: TMenuItem
          Caption = 'N_View_Coord_CopySetup'
          OnClick = N_View_Coord_CopySetupClick
        end
        object N_View_Coord_Copy: TMenuItem
          Caption = 'N_View_Coord_Copy'
          ShortCut = 67
          OnClick = N_View_Coord_CopyClick
        end
      end
      object N_S12: TMenuItem
        Caption = '-'
      end
      object N_View_ControlPanel: TMenuItem
        Caption = 'N_View_ControlPanel'
        Checked = True
        ShortCut = 115
        OnClick = N_View_FileListClick
      end
      object N_View_Refresh: TMenuItem
        Caption = 'N_View_Refresh'
        object N_View_Refresh_Slow: TMenuItem
          Caption = 'N_View_Refresh_Slow'
          RadioItem = True
          OnClick = N_View_Refresh_SlowClick
        end
        object N_View_Refresh_Norm: TMenuItem
          Caption = 'N_View_Refresh_Norm'
          RadioItem = True
          OnClick = N_View_Refresh_SlowClick
        end
        object N_View_Refresh_Fast: TMenuItem
          Caption = 'N_View_Refresh_Fast'
          Checked = True
          RadioItem = True
          OnClick = N_View_Refresh_SlowClick
        end
      end
      object N_View_Measurement: TMenuItem
        Caption = 'N_View_Measurement'
        object N_View_Measu_Metric: TMenuItem
          Caption = 'N_View_Measu_Metric'
          Checked = True
          RadioItem = True
          OnClick = N_View_Measu_MetricClick
        end
        object N_View_Measu_British: TMenuItem
          Caption = 'N_View_Measu_British'
          RadioItem = True
          OnClick = N_View_Measu_MetricClick
        end
      end
      object N_S13: TMenuItem
        Caption = '-'
      end
      object N_View_Time: TMenuItem
        Caption = 'N_View_Time'
        object N_View_Time_ZoomIn: TMenuItem
          Caption = 'N_View_Time_ZoomIn'
          ShortCut = 16571
          OnClick = ButtonZoomInClick
        end
        object N_View_Time_ZoomOut: TMenuItem
          Caption = 'N_View_Time_ZoomOut'
          ShortCut = 16573
          OnClick = ButtonZoomInClick
        end
        object N_View_Movie_Overlap: TMenuItem
          Caption = 'N_View_Movie_Overlap'
          ShortCut = 16458
          OnClick = N_View_Movie_OverlapClick
        end
        object N_View_Time_SetA: TMenuItem
          Caption = 'N_View_Time_SetA'
          ShortCut = 16420
          OnClick = N_View_Time_SetAClick
        end
        object N_View_Time_SetB: TMenuItem
          Caption = 'N_View_Time_SetB'
          ShortCut = 16419
          OnClick = N_View_Time_SetAClick
        end
        object N_View_Time_LoopAB: TMenuItem
          Caption = 'N_View_Time_LoopAB'
          ShortCut = 16460
          OnClick = N_View_Time_LoopABClick
        end
      end
      object N_View_TimeZone: TMenuItem
        Caption = 'N_View_TimeZone'
        object N_View_TimeZone_Local: TMenuItem
          Caption = 'N_View_TimeZone_Local'
          Checked = True
          RadioItem = True
          OnClick = N_View_TimeZone_LocalClick
        end
        object N_S17: TMenuItem
          Caption = '-'
        end
      end
      object N_View_Graphics: TMenuItem
        Caption = 'N_View_Graphics'
        object N_View_Graphics_S: TMenuItem
          Caption = 'N_View_Graphics_S'
          Checked = True
          ShortCut = 49235
          OnClick = N_View_Graphics_SClick
        end
        object N_View_Graphics_H: TMenuItem
          Caption = 'N_View_Graphics_H'
          ShortCut = 49224
          OnClick = N_View_Graphics_SClick
        end
        object N_View_Graphics_G: TMenuItem
          Caption = 'N_View_Graphics_G'
          Checked = True
          ShortCut = 49223
          OnClick = N_View_Graphics_SClick
        end
        object N_View_Graphics_V: TMenuItem
          Caption = 'N_View_Graphics_V'
          ShortCut = 49238
          OnClick = N_View_Graphics_SClick
        end
        object N_View_Graphics_X: TMenuItem
          Caption = 'N_View_Graphics_X'
          ShortCut = 49240
          OnClick = N_View_Graphics_SClick
        end
        object N_View_Graphics_Y: TMenuItem
          Caption = 'N_View_Graphics_Y'
          ShortCut = 49241
          OnClick = N_View_Graphics_SClick
        end
        object N_View_Graphics_Z: TMenuItem
          Caption = 'N_View_Graphics_Z'
          ShortCut = 49242
          OnClick = N_View_Graphics_SClick
        end
        object N_S9: TMenuItem
          Caption = '-'
        end
        object N_View_Graphics_Hide: TMenuItem
          Caption = 'N_View_Graphics_Hide'
          ShortCut = 117
          OnClick = N_View_Graphics_ShowClick
        end
        object N_View_Graphics_Show: TMenuItem
          Caption = 'N_View_Graphics_Show'
          ShortCut = 118
          OnClick = N_View_Graphics_ShowClick
        end
      end
      object N_View_Details: TMenuItem
        Caption = 'N_View_Details'
        Checked = True
        ShortCut = 119
        OnClick = N_View_FileListClick
      end
      object N_S3: TMenuItem
        Caption = '-'
      end
    end
    object N_Playback: TMenuItem
      Caption = 'N_Playback'
      object N_Playback_Play: TMenuItem
        Caption = 'N_Playback_Play'
        ShortCut = 32
        OnClick = N_Playback_PlayClick
      end
      object N_Playback_Stop: TMenuItem
        Caption = 'N_Playback_Stop'
        ShortCut = 27
        OnClick = N_Playback_StopClick
      end
      object N_FullScreen: TMenuItem
        Caption = 'N_FullScreen'
        ShortCut = 32781
        OnClick = N_FullScreenClick
      end
      object N_S19: TMenuItem
        Caption = '-'
      end
      object N_Playback_Left: TMenuItem
        Caption = 'N_Playback_Left'
        ShortCut = 37
        OnClick = N_Playback_LeftClick
      end
      object N_Playback_Right: TMenuItem
        Caption = 'N_Playback_Right'
        ShortCut = 39
        OnClick = N_Playback_LeftClick
      end
      object N_Playback_Frame_Prev: TMenuItem
        Caption = 'N_Playback_Frame_Prev'
        ShortCut = 66
        OnClick = ButtonFramePrevClick
      end
      object N_Playback_Frame_Next: TMenuItem
        Caption = 'N_Playback_Frame_Next'
        ShortCut = 78
        OnClick = ButtonFrameNextClick
      end
      object N_S5: TMenuItem
        Caption = '-'
      end
      object N_Playback_PauseOnClick: TMenuItem
        Caption = 'N_Playback_PauseOnClick'
        OnClick = N_Playback_PauseOnClickClick
      end
      object N_Playback_PlayNext: TMenuItem
        Caption = 'N_Playback_PlayNext'
        OnClick = N_Playback_PlayNextClick
      end
      object N_S4: TMenuItem
        Caption = '-'
      end
      object N_Playback_Save_Frame: TMenuItem
        Caption = 'N_Playback_Save_Frame'
        ShortCut = 32851
        OnClick = N_Playback_Save_FrameClick
      end
      object N_Playback_Save_Wind: TMenuItem
        Caption = 'N_Playback_Save_Wind'
        ShortCut = 32855
        OnClick = N_Playback_Save_FrameClick
      end
      object N_S6: TMenuItem
        Caption = '-'
      end
      object N_Playback_Splitters: TMenuItem
        Caption = 'N_Playback_Splitters'
        object N_Playback_Splitter_Int_MOV: TMenuItem
          Caption = 'N_Playback_Splitter_Int_MOV'
          Checked = True
          OnClick = N_Playback_Decoder_MSVideo_ForceClick
        end
        object N_Playback_Splitter_Int_AVI: TMenuItem
          Caption = 'N_Playback_Splitter_Int_AVI'
          Enabled = False
          OnClick = N_Playback_Decoder_MSVideo_ForceClick
        end
        object N_S22: TMenuItem
          Caption = '-'
        end
        object N_Playback_Splitter_DSShow: TMenuItem
          Caption = 'N_Playback_Splitter_DSShow'
          Checked = True
          OnClick = N_Playback_Decoder_MSVideo_ForceClick
        end
      end
      object N_Playback_Decoders: TMenuItem
        Caption = 'N_Playback_Decoders'
        object N_Playback_Decoder_MSVideo_Force: TMenuItem
          Caption = 'N_Playback_Decoder_MSVideo_Force'
          GroupIndex = 1
          RadioItem = True
          OnClick = N_Playback_Decoder_MSVideo_ForceClick
        end
        object N_Playback_Decoder_MSVideo_Auto: TMenuItem
          Caption = 'N_Playback_Decoder_MSVideo_Auto'
          Checked = True
          GroupIndex = 1
          RadioItem = True
          OnClick = N_Playback_Decoder_MSVideo_ForceClick
        end
        object N_Playback_Decoder_MSVideo_Never: TMenuItem
          Caption = 'N_Playback_Decoder_MSVideo_Never'
          GroupIndex = 1
          RadioItem = True
          OnClick = N_Playback_Decoder_MSVideo_ForceClick
        end
        object N_S21: TMenuItem
          Caption = '-'
          GroupIndex = 1
        end
        object N_Playback_Decoder_MSAudio_Force: TMenuItem
          Caption = 'N_Playback_Decoder_MSAudio_Force'
          GroupIndex = 2
          RadioItem = True
          OnClick = N_Playback_Decoder_MSVideo_ForceClick
        end
        object N_Playback_Decoder_MSAudio_Auto: TMenuItem
          Caption = 'N_Playback_Decoder_MSAudio_Auto'
          Checked = True
          GroupIndex = 2
          RadioItem = True
          OnClick = N_Playback_Decoder_MSVideo_ForceClick
        end
        object N_Playback_Decoder_MSAudio_Never: TMenuItem
          Caption = 'N_Playback_Decoder_MSAudio_Never'
          GroupIndex = 2
          RadioItem = True
          OnClick = N_Playback_Decoder_MSVideo_ForceClick
        end
      end
      object N_Playback_Renderer: TMenuItem
        Caption = 'N_Playback_Renderer'
        object N_Playback_Renderer_NUL: TMenuItem
          Caption = 'N_Playback_Renderer_NUL'
          GroupIndex = 1
          RadioItem = True
          OnClick = N_Playback_Renderer_NULClick
        end
        object N_Playback_Renderer_WMR: TMenuItem
          Caption = 'N_Playback_Renderer_WMR'
          GroupIndex = 1
          RadioItem = True
          OnClick = N_Playback_Renderer_NULClick
        end
        object N_Playback_Renderer_EVR: TMenuItem
          Caption = 'N_Playback_Renderer_EVR'
          Checked = True
          GroupIndex = 1
          RadioItem = True
          OnClick = N_Playback_Renderer_NULClick
        end
      end
    end
    object N_VideoZoom: TMenuItem
      Caption = 'N_VideoZoom'
      object N_VideoZoom_Custom: TMenuItem
        Caption = 'N_VideoZoom_Custom'
        GroupIndex = 1
        RadioItem = True
        OnClick = N_VideoZoom_LetterboxClick
      end
      object N_S7: TMenuItem
        Caption = '-'
        GroupIndex = 1
      end
      object N_VideoZoom_ZoomIn: TMenuItem
        Caption = 'N_VideoZoom_ZoomIn'
        GroupIndex = 1
        OnClick = ButtonVideoZoomOutClick
      end
      object N_VideoZoom_ZoomOut: TMenuItem
        Caption = 'N_VideoZoom_ZoomOut'
        GroupIndex = 1
        OnClick = ButtonVideoZoomOutClick
      end
      object N_S8: TMenuItem
        Caption = '-'
        GroupIndex = 1
      end
      object N_VideoZoom_800: TMenuItem
        Caption = '800%'
        GroupIndex = 1
        OnClick = ButtonVideoZoomOutClick
      end
      object N_VideoZoom_400: TMenuItem
        Caption = '400%'
        GroupIndex = 1
        OnClick = ButtonVideoZoomOutClick
      end
      object N_VideoZoom_200: TMenuItem
        Caption = '200%'
        GroupIndex = 1
        OnClick = ButtonVideoZoomOutClick
      end
      object N_VideoZoom_100: TMenuItem
        Caption = '100%'
        GroupIndex = 1
        OnClick = ButtonVideoZoomOutClick
      end
      object N_VideoZoom_50: TMenuItem
        Caption = '50%'
        GroupIndex = 1
        OnClick = ButtonVideoZoomOutClick
      end
      object N_VideoZoom_25: TMenuItem
        Caption = '25%'
        GroupIndex = 1
        OnClick = ButtonVideoZoomOutClick
      end
      object N_VideoZoom_10: TMenuItem
        Caption = '10%'
        GroupIndex = 1
        OnClick = ButtonVideoZoomOutClick
      end
      object N_VideoZoom_Other: TMenuItem
        Caption = 'N_VideoZoom_Other'
        GroupIndex = 1
        OnClick = N_VideoZoom_OtherClick
      end
      object N_VideoZoom_Letterbox: TMenuItem
        Break = mbBarBreak
        Caption = 'N_VideoZoom_Letterbox'
        Checked = True
        GroupIndex = 1
        RadioItem = True
        ShortCut = 32817
        OnClick = N_VideoZoom_LetterboxClick
      end
      object N_VideoZoom_PanAndScan: TMenuItem
        Caption = 'N_VideoZoom_PanAndScan'
        GroupIndex = 1
        RadioItem = True
        ShortCut = 32818
        OnClick = N_VideoZoom_LetterboxClick
      end
      object N_VideoZoom_Stretch: TMenuItem
        Caption = 'N_VideoZoom_Stretch'
        GroupIndex = 1
        RadioItem = True
        ShortCut = 32819
        OnClick = N_VideoZoom_LetterboxClick
      end
      object N_VideoZoom_NotLinear: TMenuItem
        Caption = 'N_VideoZoom_NotLinear'
        GroupIndex = 1
        RadioItem = True
        ShortCut = 32820
        OnClick = N_VideoZoom_LetterboxClick
      end
      object N_S10: TMenuItem
        Caption = '-'
        GroupIndex = 1
      end
      object N_VideoZoom_FlipV: TMenuItem
        Caption = 'N_VideoZoom_FlipV'
        GroupIndex = 1
        ShortCut = 120
        OnClick = N_VideoZoom_LetterboxClick
      end
      object N_VideoZoom_FlipH: TMenuItem
        Caption = 'N_VideoZoom_FlipH'
        GroupIndex = 1
        ShortCut = 121
        OnClick = N_VideoZoom_LetterboxClick
      end
    end
    object N_Help: TMenuItem
      Caption = 'N_Help'
      object N_Help_About: TMenuItem
        Caption = 'N_Help_About'
        ShortCut = 112
        OnClick = N_Help_AboutClick
      end
      object N_Help_WhatsNew: TMenuItem
        Caption = 'N_Help_WhatsNew'
        OnClick = N_Help_WhatsNewClick
      end
      object N_S16: TMenuItem
        Caption = '-'
      end
      object N_Help_Site_Brand: TMenuItem
        Caption = 'N_Help_Site_Brand'
        OnClick = N_Help_Site_BrandClick
      end
      object N_Help_Site_My: TMenuItem
        Caption = 'N_Help_Site_My'
        OnClick = N_Help_Site_MyClick
      end
    end
  end
  object OpenDialogFileAdder: TOpenDialog
    Filter = 'DashCam Files (*.mov, *.mp4)|*.mov;*.mp4;*.mp4v|Video Files|*.3GP;*.ASF;*.AVI;*.AVS;*.CME;*.COF;*.CSA;*.DIVX;*.FLV;*.M1V;*.M2V;*.MLS;*.MOV;*.MP4;*.MPE;*.MPEG;*.MPG;*.OGM;*.MKV;*.XFL;*.RM;*.RMVB;*.VIV;*.VOB;*.WMV;*.XQT;*.VP3;*.VP4;*.VP5;*.VP6;*.VP7|Any Files|*.*'
    Options = [ofHideReadOnly, ofAllowMultiSelect, ofEnableSizing]
    Left = 560
    Top = 80
  end
  object FilterEVR: TFilter
    BaseFilter.data = {
      C600000037D415438C5BD011BD3B00A0C911CE86B20000004000640065007600
      6900630065003A00730077003A007B0030003800330038003600330046003100
      2D0037003000440045002D0031003100440030002D0042004400340030002D00
      3000300041003000430039003100310043004500380036007D005C007B004600
      41003100300037003400360043002D0039004200360033002D00340042003600
      43002D0042004300340039002D00460043003300300030004500410035004600
      3200350036007D000000
    }
    Left = 168
    Top = 120
  end
  object PopupMenuVideoWindow: TPopupMenu
    AutoHotkeys = maManual
    Left = 200
    Top = 120
  end
  object PopupMenuListView: TPopupMenu
    AutoHotkeys = maManual
    OnPopup = PopupMenuListViewPopup
    Left = 480
    Top = 80
    object N_FileList_Open: TMenuItem
      Caption = 'N_FileList_Open'
      Default = True
      ShortCut = 13
      OnClick = ListViewDblClick
    end
    object N_SFL1: TMenuItem
      Caption = '-'
    end
    object N_FileList_Add: TMenuItem
      Caption = 'N_FileList_Add'
      ShortCut = 45
      OnClick = N_File_OpenClick
    end
    object N_FileList_Del: TMenuItem
      Caption = 'N_FileList_Del'
      ShortCut = 46
      OnClick = N_FileList_DelClick
    end
    object N_FileList_Refresh: TMenuItem
      Caption = 'N_FileList_Refresh'
      ShortCut = 116
      OnClick = N_FileList_RefreshClick
    end
    object N_FileList_Clear: TMenuItem
      Caption = 'N_FileList_Clear'
      ShortCut = 16462
      OnClick = N_FileList_ClearClick
    end
    object N_SFL2: TMenuItem
      Caption = '-'
    end
    object N_FileList_Copy: TMenuItem
      Caption = 'N_FileList_Copy'
      ShortCut = 8259
      OnClick = N_FileList_CopyClick
    end
    object N_FileList_DelDisk: TMenuItem
      Caption = 'N_FileList_DelDisk'
      ShortCut = 8238
      OnClick = N_FileList_DelDiskClick
    end
    object N_FileList_DelDiskAll: TMenuItem
      Caption = 'N_FileList_DelDiskAll'
      OnClick = N_FileList_DelDiskAllClick
    end
    object N_FileList_Save: TMenuItem
      Caption = 'N_FileList_Save'
      OnClick = N_FileList_SaveClick
    end
    object N_FileList_Repair: TMenuItem
      Caption = 'N_FileList_Repair'
      OnClick = N_FileList_RepairClick
    end
    object N_SFL3: TMenuItem
      Caption = '-'
    end
    object N_FileList_SetLock: TMenuItem
      Caption = 'N_FileList_SetLock'
      OnClick = N_FileList_SetLockClick
    end
    object N_FileList_UnsetLock: TMenuItem
      Caption = 'N_FileList_UnsetLock'
      OnClick = N_FileList_SetLockClick
    end
    object N_SFL4: TMenuItem
      Caption = '-'
    end
    object N_FileList_Group_With: TMenuItem
      Caption = 'N_FileList_Group_With'
      OnClick = N_FileList_Group_WithClick
    end
    object N_FileList_Ungroup: TMenuItem
      Caption = 'N_FileList_Ungroup'
      OnClick = N_FileList_UngroupClick
    end
    object N_FileList_Group_Auto: TMenuItem
      Caption = 'N_FileList_Group_Auto'
      OnClick = N_FileList_Group_AutoClick
    end
    object N_FileList_Ungroup_All: TMenuItem
      Caption = 'N_FileList_Ungroup_All'
      OnClick = N_FileList_Ungroup_AllClick
    end
    object N_SFL5: TMenuItem
      Caption = '-'
    end
    object N_FileList_Track_Google: TMenuItem
      Caption = 'N_FileList_Track_Google'
      OnClick = N_FileList_Track_GoogleClick
    end
    object N_FileList_Track_Export: TMenuItem
      Caption = 'N_FileList_Track_Export'
      OnClick = N_FileList_Track_ExportClick
    end
    object N_FileList_Track_Set: TMenuItem
      Caption = 'N_FileList_Track_Set'
      OnClick = N_FileList_Track_SetClick
    end
    object N_FileList_Track_GPSLog: TMenuItem
      Caption = 'N_FileList_Track_GPSLog'
      Visible = False
      OnClick = N_FileList_Track_GPSLogClick
    end
    object N_FileList_Track_Move: TMenuItem
      Caption = 'N_FileList_Track_Move'
      OnClick = N_FileList_Track_MoveClick
    end
    object N_SFL6: TMenuItem
      Caption = '-'
    end
    object N_FileList_ShowExplorer: TMenuItem
      Caption = 'N_FileList_ShowExplorer'
      OnClick = N_FileList_ShowExplorerClick
    end
    object N_FileList_Property: TMenuItem
      Caption = 'N_FileList_Property'
      OnClick = N_FileList_PropertyClick
    end
  end
  object ImageListView: TImageList
    Left = 520
    Top = 80
  end
  object SaveDialogJpeg: TSaveDialog
    DefaultExt = 'jpg'
    Filter = 'Jpeg Files (.jpg;.jpeg)|*.jpg;*.jpeg'
    Options = [ofOverwritePrompt, ofHideReadOnly, ofEnableSizing]
    Left = 264
    Top = 120
  end
  object SaveDialogGpsConverter: TSaveDialog
    Options = [ofOverwritePrompt, ofHideReadOnly, ofEnableSizing]
    Left = 528
    Top = 272
  end
  object ApplicationEvents: TApplicationEvents
    OnException = ApplicationEventsException
    OnMessage = ApplicationEventsMessage
    OnShortCut = ApplicationEventsShortCut
    Left = 128
    Top = 8
  end
  object TimerMouseHide: TTimer
    Enabled = False
    Interval = 2000
    OnTimer = TimerMouseHideTimer
    Left = 232
    Top = 120
  end
  object PopupMenuGraphics: TPopupMenu
    AutoHotkeys = maManual
    OnPopup = PopupMenuGraphicsPopup
    Left = 176
    Top = 469
  end
  object TimerOnStart: TTimer
    Interval = 1
    OnTimer = TimerOnStartTimer
    Left = 24
    Top = 112
  end
  object OpenDialogTrack: TOpenDialog
    Filter = 'All supported formats|*.dat;*.log;*.plt;*.gpx|*.dat|*.dat|*.log|*.log|*.plt|*.plt|*.gpx|*.gpx|All files|*.*'
    Left = 495
    Top = 272
  end
  object FilterMSVideo: TFilter
    BaseFilter.data = {
      C600000037D415438C5BD011BD3B00A0C911CE86B20000004000640065007600
      6900630065003A00730077003A007B0030003800330038003600330046003100
      2D0037003000440045002D0031003100440030002D0042004400340030002D00
      3000300041003000430039003100310043004500380036007D005C007B003200
      31003200360039003000460042002D0038003300450035002D00340035003200
      36002D0038004600440037002D00370034003400370038004200370039003300
      3900430044007D000000
    }
    Left = 168
    Top = 152
  end
  object FilterMSAudio: TFilter
    BaseFilter.data = {
      C600000037D415438C5BD011BD3B00A0C911CE86B20000004000640065007600
      6900630065003A00730077003A007B0030003800330038003600330046003100
      2D0037003000440045002D0031003100440030002D0042004400340030002D00
      3000300041003000430039003100310043004500380036007D005C007B004500
      31004600310041003000420038002D0042004500450045002D00340039003000
      44002D0042004100370043002D00300036003600430034003000420035004500
      3200420039007D000000
    }
    Left = 168
    Top = 184
  end
  object OpenDialogKML: TOpenDialog
    Filter = 'All supported formats|*.kml;*.gpx|KML|*.kml|GPX|*.GPX|All files|*.*'
    Left = 498
    Top = 304
  end
end
